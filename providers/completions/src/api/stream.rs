use std::collections::BTreeMap;

use async_stream::try_stream;
use futures::StreamExt;
use futures::stream::BoxStream;
use serde::Deserialize;
use serde_json::{Value, json};
use tools_rs::FunctionCall;

use chat_core::{
    error::ChatError,
    traits::StreamProvider,
    transport::Transport,
    types::{
        messages::{
            Messages,
            content::{Content, RoleEnum},
            parts::{PartEnum, Parts},
            text::Text,
        },
        metadata::{Metadata, usage::Usage},
        options::ChatOptions,
        response::{ChatResponse, StreamEvent},
        tools::ToolDeclarations,
    },
};

use crate::{
    api::types::{
        request::{CompletionsRequest, CompletionsRequestConfig},
        response::{CompletionsUsage, finish_reason_to_core},
    },
    client::CompletionsClient,
};

#[async_trait::async_trait]
impl<T: Transport> StreamProvider for CompletionsClient<T> {
    async fn stream(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError> {
        let mut request_body = CompletionsRequest::from_core(CompletionsRequestConfig {
            model_name: &self.model_name,
            messages,
            tool_declarations,
            options,
            output_shape: None,
        })?;
        request_body.stream = Some(true);
        request_body.stream_options = Some(json!({"include_usage": true}));

        let body = serde_json::to_vec(&request_body)
            .map_err(|e| ChatError::InvalidResponse(e.to_string()))?;

        let req = chat_core::transport::Request {
            scheme: self.scheme.clone(),
            host: self.host.clone(),
            path: format!("{}/chat/completions", self.base_path),
            headers: self.build_headers(),
            body,
        };

        let event_stream = self.transport.stream(req).await.map_err(ChatError::from)?;
        Ok(parse_event_stream(event_stream))
    }
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    choices: Vec<StreamChoice>,
    #[serde(default)]
    usage: Option<CompletionsUsage>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: StreamDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct StreamDelta {
    #[serde(default)]
    content: Option<Value>,
    /// Thinking-model channel used by Qwen3, DeepSeek-R1 and clones.
    /// Both `reasoning_content` and `reasoning` are accepted as aliases.
    #[serde(default, alias = "reasoning")]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<StreamToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct StreamToolCallDelta {
    #[serde(default)]
    index: Option<usize>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    function: Option<StreamToolCallFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct StreamToolCallFunctionDelta {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Default)]
struct ToolCallState {
    id: Option<String>,
    name: String,
    arguments: String,
    announced: bool,
}

#[derive(Default)]
struct StreamState {
    text_buf: String,
    reasoning_buf: String,
    tool_calls: BTreeMap<usize, ToolCallState>,
    finish_reason: Option<String>,
    id: Option<String>,
    model: Option<String>,
    usage: Option<Usage>,
}

impl StreamState {
    fn handle_chunk(&mut self, chunk: StreamChunk) -> Vec<StreamEvent> {
        let mut events = Vec::new();

        if self.id.is_none() && chunk.id.is_some() {
            self.id = chunk.id;
        }
        if self.model.is_none() && chunk.model.is_some() {
            self.model = chunk.model;
        }
        if let Some(usage) = chunk.usage {
            self.usage = Some(usage.to_core());
        }

        for choice in chunk.choices {
            if let Some(reason) = choice.finish_reason {
                self.finish_reason = Some(reason);
            }

            if let Some(content) = choice.delta.content
                && let Some(text) = content.as_str()
                && !text.is_empty()
            {
                self.text_buf.push_str(text);
                events.push(StreamEvent::TextChunk(text.to_string()));
            }

            if let Some(reasoning) = choice.delta.reasoning_content
                && !reasoning.is_empty()
            {
                self.reasoning_buf.push_str(&reasoning);
                events.push(StreamEvent::ReasoningChunk(reasoning));
            }

            if let Some(deltas) = choice.delta.tool_calls {
                for (idx_fallback, delta) in deltas.into_iter().enumerate() {
                    let index = delta.index.unwrap_or(idx_fallback);
                    let state = self.tool_calls.entry(index).or_default();

                    if let Some(id) = delta.id
                        && state.id.is_none()
                    {
                        state.id = Some(id);
                    }
                    if let Some(func) = delta.function {
                        if let Some(name) = func.name
                            && state.name.is_empty()
                        {
                            state.name = name;
                        }
                        if let Some(args) = func.arguments {
                            state.arguments.push_str(&args);
                        }
                    }

                    if !state.announced && !state.name.is_empty() {
                        state.announced = true;
                        events.push(StreamEvent::ToolCall(FunctionCall {
                            id: state.id.clone().map(Into::into),
                            name: state.name.clone(),
                            arguments: Value::Null,
                        }));
                    }
                }
            }
        }

        events
    }

    fn into_response(self) -> ChatResponse {
        use chat_core::types::messages::reasoning::Reasoning;
        let mut parts = Parts::default();

        if !self.reasoning_buf.is_empty() {
            parts.push(PartEnum::Reasoning(Reasoning::new(self.reasoning_buf)));
        }

        if !self.text_buf.is_empty() {
            if let Ok(parsed) = serde_json::from_str::<Value>(&self.text_buf)
                && (parsed.is_object() || parsed.is_array())
            {
                parts.push(PartEnum::Structured(parsed));
            } else {
                parts.push(PartEnum::Text(Text::new(self.text_buf)));
            }
        }

        let mut had_tool_calls = false;
        for (_idx, state) in self.tool_calls {
            had_tool_calls = true;
            let arguments: Value = if state.arguments.is_empty() {
                Value::Null
            } else {
                serde_json::from_str(&state.arguments).unwrap_or(Value::Null)
            };
            parts.push(PartEnum::from_function_call(FunctionCall {
                id: state.id.map(Into::into),
                name: state.name,
                arguments,
            }));
        }

        let complete_reason = finish_reason_to_core(self.finish_reason.as_deref(), had_tool_calls);

        ChatResponse {
            content: Content {
                role: RoleEnum::Model,
                parts,
                complete_reason,
            },
            metadata: Some(Metadata {
                id: self.id,
                model_slug: self.model,
                usage: self.usage.unwrap_or_default(),
                ..Default::default()
            }),
        }
    }
}

fn parse_event_stream(
    event_stream: chat_core::transport::EventStream,
) -> BoxStream<'static, Result<StreamEvent, ChatError>> {
    let stream = try_stream! {
        let mut events = event_stream;
        let mut state = StreamState::default();

        while let Some(event_res) = events.next().await {
            let (_event_type, data) = event_res.map_err(ChatError::from)?;

            if data.is_empty() {
                continue;
            }

            let chunk: StreamChunk = match serde_json::from_str(&data) {
                Ok(c) => c,
                Err(e) => Err(ChatError::InvalidResponse(format!(
                    "stream chunk parse: {e}; payload: {data}"
                )))?,
            };

            for evt in state.handle_chunk(chunk) {
                yield evt;
            }
        }

        yield StreamEvent::Done(state.into_response());
    };

    Box::pin(stream)
}
