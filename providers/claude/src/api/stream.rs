use async_stream::try_stream;
use futures::{StreamExt, stream::BoxStream};

use chat_core::{
    error::ChatError,
    traits::StreamProvider,
    types::{
        messages::{
            Messages,
            content::{CompleteReasonEnum, Content, RoleEnum},
            parts::{PartEnum, Parts},
            reasoning::Reasoning,
            text::Text,
        },
        options::ChatOptions,
        response::{ChatResponse, SseParser, StreamEvent},
    },
};
use serde_json::Value;
use tools_rs::{CallId, FunctionCall};

use crate::{
    api::types::error::handle_claude_error,
    api::types::request::ClaudeRequest,
    client::ClaudeClient,
};

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";
const THINKING_BETA_HEADER: &str = "interleaved-thinking-2025-05-14";

#[async_trait::async_trait]
impl StreamProvider for ClaudeClient {
    async fn stream(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&Value>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError> {
        let request_body = ClaudeRequest::from_core(
            &self.model_name,
            messages,
            tool_declarations,
            options,
            None,
            true,
            self.thinking_budget,
        )?;

        let mut req = self
            .http_client
            .post(CLAUDE_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", &self.api_version)
            .header("content-type", "application/json");

        if self.include_thoughts {
            req = req.header("anthropic-beta", THINKING_BETA_HEADER);
        }

        let res = req
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ChatError::Network(e.to_string()))?;

        let res = handle_claude_error(res).await.map_err(|f| f.err)?;

        Ok(parse_claude_sse_stream(res))
    }
}

/// Convert a Claude SSE event into a PartEnum for merge_chunk.
fn sse_event_to_part(event_type: &str, json_str: &str) -> Result<Option<PartEnum>, ChatError> {
    let data: Value = serde_json::from_str(json_str)
        .map_err(|e| ChatError::InvalidResponse(format!("Failed to parse SSE data: {e}")))?;

    match event_type {
        "content_block_delta" => {
            let delta = data.get("delta");
            let delta_type = delta
                .and_then(|d| d.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("");

            match delta_type {
                "text_delta" => {
                    let text = delta
                        .and_then(|d| d.get("text"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("");
                    Ok(Some(PartEnum::Text(Text::new(text))))
                }
                "thinking_delta" => {
                    let thinking = delta
                        .and_then(|d| d.get("thinking"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("");
                    Ok(Some(PartEnum::Reasoning(Reasoning::new(thinking))))
                }
                "signature_delta" => {
                    let sig = delta
                        .and_then(|d| d.get("signature"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("");
                    // Signature comes as a separate delta — create a reasoning part with
                    // just the signature so merge_chunk can attach it.
                    Ok(Some(PartEnum::Reasoning(
                        Reasoning::new("").with_signature(sig.to_string()),
                    )))
                }
                "input_json_delta" => {
                    // Tool input streaming — accumulate as text so merge_chunk concatenates.
                    // We'll reconstruct the FunctionCall at content_block_stop.
                    Ok(None)
                }
                _ => Ok(None),
            }
        }
        "content_block_start" => {
            let cb = data.get("content_block");
            let block_type = cb
                .and_then(|c| c.get("type"))
                .and_then(|t| t.as_str())
                .unwrap_or("");

            match block_type {
                "tool_use" => {
                    let id = cb
                        .and_then(|c| c.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let name = cb
                        .and_then(|c| c.get("name"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    Ok(Some(PartEnum::from_function_call(FunctionCall {
                        id: Some(CallId::from(id)),
                        name,
                        arguments: Value::Null,
                    })))
                }
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

fn parse_claude_sse_stream(
    res: reqwest::Response,
) -> BoxStream<'static, Result<StreamEvent, ChatError>> {
    let stream = try_stream! {
        let mut byte_stream = res.bytes_stream();
        let mut sse_parser = SseParser::default();

        let mut final_parts = Parts::default();
        let mut final_reason = CompleteReasonEnum::None;
        let mut message_id: Option<String> = None;
        let mut model: Option<String> = None;
        let mut input_tokens: usize = 0;
        let mut output_tokens: usize = 0;

        // For accumulating tool input JSON across deltas
        let mut tool_input_buffer = String::new();

        while let Some(chunk_res) = byte_stream.next().await {
            let chunk = chunk_res.map_err(|e| ChatError::Network(e.to_string()))?;
            sse_parser.push(&chunk);

            while let Some((event_type, json_str)) = sse_parser.next_event() {
                match event_type.as_str() {
                    "message_start" => {
                        if let Ok(data) = serde_json::from_str::<Value>(&json_str) {
                            if let Some(msg) = data.get("message") {
                                message_id = msg.get("id").and_then(|v| v.as_str()).map(str::to_string);
                                model = msg.get("model").and_then(|v| v.as_str()).map(str::to_string);
                                if let Some(u) = msg.get("usage") {
                                    input_tokens = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                }
                            }
                        }
                    }
                    "content_block_start" => {
                        // Check if it's a tool_use block to start accumulating input
                        if let Ok(data) = serde_json::from_str::<Value>(&json_str) {
                            let block_type = data.get("content_block")
                                .and_then(|c| c.get("type"))
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            if block_type == "tool_use" {
                                tool_input_buffer.clear();
                            }
                        }
                        if let Ok(Some(part)) = sse_event_to_part("content_block_start", &json_str) {
                            if let Some(event) = final_parts.merge_chunk(part) {
                                yield event;
                            }
                        }
                    }
                    "content_block_delta" => {
                        // Handle tool input accumulation separately
                        if let Ok(data) = serde_json::from_str::<Value>(&json_str) {
                            let delta_type = data.get("delta")
                                .and_then(|d| d.get("type"))
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            if delta_type == "input_json_delta" {
                                if let Some(partial) = data.get("delta")
                                    .and_then(|d| d.get("partial_json"))
                                    .and_then(|v| v.as_str()) {
                                    tool_input_buffer.push_str(partial);
                                }
                                continue;
                            }
                        }

                        if let Ok(Some(part)) = sse_event_to_part("content_block_delta", &json_str) {
                            if let Some(event) = final_parts.merge_chunk(part) {
                                yield event;
                            }
                        }
                    }
                    "content_block_stop" => {
                        // If we were accumulating tool input, finalize the Tool part's
                        // call arguments and emit the ToolCall event.
                        if !tool_input_buffer.is_empty() {
                            let input: Value = serde_json::from_str(&tool_input_buffer)
                                .unwrap_or(Value::Object(Default::default()));
                            if let Some(PartEnum::Tool(tool)) = final_parts.0.last_mut() {
                                tool.call.arguments = input;
                                let event = StreamEvent::ToolCall(tool.call.clone());
                                yield event;
                            }
                            tool_input_buffer.clear();
                        }
                    }
                    "message_delta" => {
                        if let Ok(data) = serde_json::from_str::<Value>(&json_str) {
                            if let Some(sr) = data.get("stop_reason").and_then(|v| v.as_str()) {
                                final_reason = match sr {
                                    "end_turn" => CompleteReasonEnum::Stop,
                                    "max_tokens" => CompleteReasonEnum::MaxTokens,
                                    "tool_use" => CompleteReasonEnum::ToolCall,
                                    other => CompleteReasonEnum::Other(other.to_string()),
                                };
                            }
                            if let Some(u) = data.get("usage") {
                                output_tokens = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                            }
                        }
                    }
                    "error" => {
                        match serde_json::from_str::<Value>(&json_str) {
                            Ok(data) => {
                                let msg = data.get("error")
                                    .and_then(|e| e.get("message"))
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("Unknown stream error");
                                Err(ChatError::Provider(msg.to_string()))?;
                            }
                            Err(_) => {
                                Err(ChatError::Provider(format!("Stream error (unparseable): {}", json_str)))?;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let metadata = chat_core::types::metadata::Metadata {
            id: message_id,
            model_slug: model,
            usage: chat_core::types::metadata::usage::Usage {
                input_tokens,
                output_tokens,
                total_tokens: input_tokens + output_tokens,
            },
            ..Default::default()
        };

        yield StreamEvent::Done(ChatResponse {
            content: Content {
                role: RoleEnum::Model,
                parts: final_parts,
                complete_reason: final_reason,
            },
            metadata: Some(metadata),
        });
    };

    Box::pin(stream)
}
