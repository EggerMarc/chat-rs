use std::collections::HashMap;

use async_stream::try_stream;
use futures::{StreamExt, stream::BoxStream};
use serde_json::Value;
use tools_rs::{FunctionCall, ToolCollection};

use chat_core::{
    error::ChatError,
    traits::StreamProvider,
    types::{
        messages::{
            Messages,
            content::{CompleteReasonEnum, Content, RoleEnum},
            parts::{PartEnum, Parts},
        },
        metadata::usage::Usage,
        options::ChatOptions,
        response::{ChatResponse, SseParser, StreamEvent},
    },
};

use crate::{
    api::types::{
        error::handle_openai_error,
        parts::{OpenAIContent, OpenAIPartEnum},
        request::OpenAIRequest,
        response::OpenAIResponse,
    },
    client::OpenAIClient,
};

#[async_trait::async_trait]
impl StreamProvider for OpenAIClient {
    async fn stream(
        &self,
        messages: &mut Messages,
        tools: Option<&ToolCollection>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut request_body = OpenAIRequest::from_core(
            &self.model_name,
            messages,
            tools,
            self.native_tools.as_slice(),
            self.reasoning_effort.clone(),
            options,
            None,
        )?;

        // Enable streaming
        request_body.stream = Some(true);
        let res = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ChatError::Network(e.to_string()))?;

        let res = handle_openai_error(res)
            .await
            .map_err(|failure| failure.err)?;

        Ok(parse_openai_sse_stream(res))
    }
}

fn parse_openai_sse_stream(
    res: reqwest::Response,
) -> BoxStream<'static, Result<StreamEvent, ChatError>> {
    let stream = try_stream! {
        let mut byte_stream = res.bytes_stream();
        let mut sse_parser = SseParser::default();

        let mut final_parts = Parts::default();
        let mut final_reason = CompleteReasonEnum::None;
        let mut final_metadata = None;
        // Maps OpenAI tool-call index -> position in final_parts.0
        let mut tool_index_map: HashMap<usize, usize> = HashMap::new();

        while let Some(chunk_res) = byte_stream.next().await {
            let chunk = chunk_res.map_err(|e| ChatError::Network(e.to_string()))?;

            sse_parser.push(&chunk);

            while let Some(json_str) = sse_parser.next_event() {
                // OpenAI signals end-of-stream with [DONE]
                if json_str.trim() == "[DONE]" {
                    continue;
                }

                // Reuse the same OpenAIResponse type — streaming chunks have
                // "delta" instead of "message", handled by #[serde(alias)].
                let mut oai_resp = serde_json::from_str::<OpenAIResponse>(&json_str)
                    .map_err(|e| {
                        ChatError::InvalidResponse(format!("Failed to parse OpenAI SSE chunk: {e}"))
                    })?;

                let mut choice = match oai_resp.choices.pop() {
                    Some(c) => c,
                    None => continue,
                };

                if choice.finish_reason.as_deref().is_some() {
                    let reason = match choice.finish_reason.as_deref() {
                        Some("stop") => CompleteReasonEnum::Stop,
                        Some("length") => CompleteReasonEnum::MaxTokens,
                        Some("tool_calls") => CompleteReasonEnum::ToolCall,
                        Some(other) => CompleteReasonEnum::Other(other.to_string()),
                        None => CompleteReasonEnum::None,
                    };
                    final_reason = reason;
                }

                if oai_resp.usage.is_some() || oai_resp.id.is_some() || oai_resp.model.is_some() {
                    final_metadata = Some(chat_core::types::metadata::Metadata {
                        id: oai_resp.id,
                        model_slug: oai_resp.model,
                        usage: oai_resp
                            .usage
                            .map(|u| Usage{
                                input_tokens: u.prompt_tokens.unwrap_or(0),
                                output_tokens: u.completion_tokens.unwrap_or(0),
                                total_tokens: u.total_tokens.unwrap_or(0),
                            })
                            .unwrap_or_default(),
                        ..Default::default()
                    });
                }

                // Extract tool calls before converting so we can
                // route them by OpenAI's stable `index` field.
                let tool_calls = choice.message.tool_calls.take();

                // Parse the response message into typed OpenAI parts
                let oai_content = OpenAIContent::from_response_message(choice.message, true)?;

                // Merge non-tool-call parts into final_parts via core merge_chunk
                for part in oai_content.parts {
                    match part {
                        OpenAIPartEnum::FunctionCall(_) => {
                            // Tool calls handled separately below with index routing
                        }
                        other => {
                            let core_part = PartEnum::from(other);
                            if let Some(event) = final_parts.merge_chunk(core_part) {
                                yield event;
                            }
                        }
                    }
                }

                // Handle tool calls with index-aware merging so that
                // interleaved chunks (A -> B -> A-cont) are routed correctly.
                if let Some(tcs) = tool_calls {
                    for tc in tcs {
                        let func = match tc.function {
                            Some(f) => f,
                            None => continue,
                        };
                        let name = func.name.unwrap_or_default();
                        let args_str = func.arguments.unwrap_or_default();
                        let arguments = Value::String(args_str);
                        let fc = FunctionCall {
                            id: tc.id.map(Into::into),
                            name,
                            arguments,
                        };

                        if let Some(event) = merge_tool_call_chunk(
                            &mut final_parts,
                            &mut tool_index_map,
                            fc,
                            tc.index,
                        ) {
                            yield event;
                        }
                    }
                }
            }
        }

        // After streaming, parse accumulated argument string fragments
        // into proper JSON values for each FunctionCall.
        for part in &mut final_parts.0 {
            if let PartEnum::FunctionCall(fc) = part {
                if let serde_json::Value::String(ref s) = fc.arguments {
                    fc.arguments = serde_json::from_str(s).unwrap_or_else(|_| serde_json::json!({}));
                }
            }
        }

        let final_response = ChatResponse {
            content: Content {
                role: RoleEnum::Model,
                parts: final_parts,
                complete_reason: final_reason,
            },
            metadata: final_metadata,
        };

        yield StreamEvent::Done(final_response);
    };

    Box::pin(stream)
}

/// Merge an OpenAI tool-call chunk into `parts` using index-based routing.
///
/// When `index` is `Some`, the chunk is routed to the `FunctionCall` at the
/// position recorded in `tool_index_map` (or a new entry is created).  When
/// `index` is `None`, the chunk falls through to adjacency-based
/// `Parts::merge_chunk`.
fn merge_tool_call_chunk(
    parts: &mut Parts,
    tool_index_map: &mut HashMap<usize, usize>,
    fc: FunctionCall,
    index: Option<usize>,
) -> Option<StreamEvent> {
    if let Some(idx) = index {
        if let Some(&pos) = tool_index_map.get(&idx) {
            if let Some(PartEnum::FunctionCall(existing)) = parts.0.get_mut(pos) {
                match (&mut existing.arguments, &fc.arguments) {
                    (Value::String(e), Value::String(n)) => e.push_str(n),
                    _ => existing.arguments = fc.arguments,
                }
                if existing.id.is_none() && fc.id.is_some() {
                    existing.id = fc.id;
                }
                if existing.name.is_empty() && !fc.name.is_empty() {
                    existing.name = fc.name;
                }
            }
            None
        } else {
            let pos = parts.0.len();
            tool_index_map.insert(idx, pos);
            parts.push(PartEnum::FunctionCall(fc.clone()));
            Some(StreamEvent::ToolCall(fc))
        }
    } else {
        parts.merge_chunk(PartEnum::FunctionCall(fc))
    }
}
