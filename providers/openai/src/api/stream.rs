use std::collections::HashMap;

use async_stream::try_stream;
use futures::{StreamExt, stream::BoxStream};
use serde::Deserialize;
use serde_json::Value;
use tools_rs::FunctionCall;

use chat_core::{
    error::ChatError,
    traits::StreamProvider,
    types::{
        messages::{
            Messages,
            content::{CompleteReasonEnum, Content, RoleEnum},
            parts::Parts,
        },
        metadata::{Metadata, usage::Usage},
        options::ChatOptions,
        response::{ChatResponse, SseParser, StreamEvent},
        tools::ToolDeclarations,
    },
};

use crate::{
    api::types::{
        error::handle_openai_error,
        request::OpenAIResponsesRequest,
        response::{OpenAIUsage, ResponsesOutputItem, output_items_to_parts},
    },
    client::OpenAIClient,
};

#[async_trait::async_trait]
impl StreamProvider for OpenAIClient {
    async fn stream(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError> {
        let url = format!("{}/responses", self.base_url);

        let previous_response_id = if self.use_previous_response_id {
            self.last_response_id.clone()
        } else {
            None
        };

        let mut request_body = OpenAIResponsesRequest::from_core(
            crate::api::types::request::ResponsesRequestConfig {
                model_name: &self.model_name,
                messages,
                tool_declarations,
                native_tools: self.native_tools.as_slice(),
                reasoning_effort: self.reasoning_effort.clone(),
                options,
                output_shape: None,
                previous_response_id,
                store: self.store,
            },
        )?;
        request_body.stream = Some(true);

        let res = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ChatError::Network(e.to_string()))?;
        let res = handle_openai_error(res).await.map_err(|f| f.err)?;

        Ok(parse_responses_sse_stream(res))
    }

    fn on_stream_done(&mut self, response: &ChatResponse) {
        if self.use_previous_response_id
            && let Some(ref meta) = response.metadata
            && let Some(ref id) = meta.id
        {
            self.last_response_id = Some(id.clone());
        }
    }
}

// ---------------------------------------------------------------------------
// SSE event data shapes (matching OpenAI Responses API wire format)
//
// Event reference:
//   response.created / response.completed → { response: { id, model, output, usage, status } }
//   response.output_item.added            → { item: { type, ... } }
//   response.output_text.delta            → { delta: "text chunk" }
//   response.reasoning_summary_text.delta → { delta: "reasoning chunk" }
//   response.function_call_arguments.delta→ { item_id: "...", delta: "arg fragment" }
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SseResponseWrapper {
    response: SseResponseData,
}

#[derive(Debug, Deserialize)]
struct SseResponseData {
    id: Option<String>,
    model: Option<String>,
    output: Option<Vec<ResponsesOutputItem>>,
    usage: Option<OpenAIUsage>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SseOutputItemAdded {
    item: ResponsesOutputItem,
}

/// Text and reasoning delta events have `delta` as a plain string.
#[derive(Debug, Deserialize)]
struct SseTextDelta {
    delta: String,
}

/// Function call argument delta events have `delta` as a plain string
/// and an `item_id` to correlate with the function call.
#[derive(Debug, Deserialize)]
struct SseFcArgsDelta {
    item_id: Option<String>,
    delta: String,
}

// ---------------------------------------------------------------------------
// Stream state — accumulates parts across SSE events
// ---------------------------------------------------------------------------

struct StreamState {
    parts: Parts,
    metadata: Option<Metadata>,
    reason: CompleteReasonEnum,
    fc_args: HashMap<String, String>,
}

impl Default for StreamState {
    fn default() -> Self {
        Self {
            parts: Parts::default(),
            metadata: None,
            reason: CompleteReasonEnum::None,
            fc_args: HashMap::new(),
        }
    }
}

impl StreamState {
    fn handle_event(
        &mut self,
        event_type: &str,
        data: &str,
    ) -> Result<Option<StreamEvent>, ChatError> {
        match event_type {
            "response.created" => self.on_created(data),
            "response.output_item.added" => self.on_item_added(data),
            "response.output_text.delta" => self.on_text_delta(data),
            "response.reasoning_summary_text.delta" => self.on_reasoning_delta(data),
            "response.function_call_arguments.delta" => self.on_fc_args_delta(data),
            "response.completed" => self.on_completed(data),
            _ => Ok(None),
        }
    }

    fn on_created(&mut self, data: &str) -> Result<Option<StreamEvent>, ChatError> {
        if let Ok(wrapper) = serde_json::from_str::<SseResponseWrapper>(data) {
            self.metadata = Some(Metadata {
                id: wrapper.response.id,
                ..Default::default()
            });
        }
        Ok(None)
    }

    fn on_item_added(&mut self, data: &str) -> Result<Option<StreamEvent>, ChatError> {
        let added: SseOutputItemAdded =
            serde_json::from_str(data).map_err(|e| ChatError::InvalidResponse(e.to_string()))?;

        if let ResponsesOutputItem::FunctionCall(fc) = &added.item {
            let call_id = fc.call_id.clone().unwrap_or_default();
            self.fc_args.insert(call_id, String::new());
            return Ok(Some(StreamEvent::ToolCall(FunctionCall {
                id: fc.call_id.clone().map(Into::into),
                name: fc.name.clone().unwrap_or_default(),
                arguments: Value::Null,
            })));
        }
        Ok(None)
    }

    fn on_text_delta(&mut self, data: &str) -> Result<Option<StreamEvent>, ChatError> {
        let parsed: SseTextDelta =
            serde_json::from_str(data).map_err(|e| ChatError::InvalidResponse(e.to_string()))?;
        Ok(Some(StreamEvent::TextChunk(parsed.delta)))
    }

    fn on_reasoning_delta(&mut self, data: &str) -> Result<Option<StreamEvent>, ChatError> {
        let parsed: SseTextDelta =
            serde_json::from_str(data).map_err(|e| ChatError::InvalidResponse(e.to_string()))?;
        Ok(Some(StreamEvent::ReasoningChunk(parsed.delta)))
    }

    fn on_fc_args_delta(&mut self, data: &str) -> Result<Option<StreamEvent>, ChatError> {
        let parsed: SseFcArgsDelta =
            serde_json::from_str(data).map_err(|e| ChatError::InvalidResponse(e.to_string()))?;

        let item_id = parsed.item_id.unwrap_or_default();
        if let Some(acc) = self.fc_args.get_mut(&item_id) {
            acc.push_str(&parsed.delta);
        } else if let Some(acc) = self.fc_args.values_mut().last() {
            acc.push_str(&parsed.delta);
        }
        Ok(None)
    }

    fn on_completed(&mut self, data: &str) -> Result<Option<StreamEvent>, ChatError> {
        let wrapper: SseResponseWrapper =
            serde_json::from_str(data).map_err(|e| ChatError::InvalidResponse(e.to_string()))?;

        let completed = wrapper.response;
        let output = completed.output.unwrap_or_default();

        let (parts, has_fc) = output_items_to_parts(&output);
        self.parts.extend(parts);

        self.reason = if has_fc {
            CompleteReasonEnum::ToolCall
        } else {
            match completed.status.as_deref() {
                Some("completed") => CompleteReasonEnum::Stop,
                Some("incomplete") => CompleteReasonEnum::MaxTokens,
                Some(other) => CompleteReasonEnum::Other(other.to_string()),
                _ => CompleteReasonEnum::None,
            }
        };

        self.metadata = Some(Metadata {
            id: completed.id,
            model_slug: completed.model,
            usage: completed
                .usage
                .map(|u| Usage {
                    input_tokens: u.input_tokens.unwrap_or(0),
                    output_tokens: u.output_tokens.unwrap_or(0),
                    total_tokens: u.total_tokens.unwrap_or(0),
                })
                .unwrap_or_default(),
            ..Default::default()
        });

        Ok(None)
    }

    fn into_response(self) -> ChatResponse {
        ChatResponse {
            content: Content {
                role: RoleEnum::Model,
                parts: self.parts,
                complete_reason: self.reason,
            },
            metadata: self.metadata,
        }
    }
}

fn parse_responses_sse_stream(
    res: reqwest::Response,
) -> BoxStream<'static, Result<StreamEvent, ChatError>> {
    let stream = try_stream! {
        let mut byte_stream = res.bytes_stream();
        let mut sse_parser = SseParser::default();
        let mut state = StreamState::default();

        while let Some(chunk_res) = byte_stream.next().await {
            let chunk = chunk_res.map_err(|e| ChatError::Network(e.to_string()))?;
            sse_parser.push(&chunk);

            while let Some((event_type, data)) = sse_parser.next_event() {
                if let Some(event) = state.handle_event(&event_type, &data)? {
                    yield event;
                }
            }
        }

        yield StreamEvent::Done(state.into_response());
    };

    Box::pin(stream)
}
