use async_stream::try_stream;
use futures::{StreamExt, stream::BoxStream};

use chat_core::{
    error::ChatError,
    traits::StreamProvider,
    transport::Transport,
    types::{
        messages::{
            Messages,
            content::{CompleteReasonEnum, Content, RoleEnum},
            parts::{PartEnum, Parts},
            reasoning::Reasoning,
            text::Text,
        },
        options::ChatOptions,
        response::{ChatResponse, StreamEvent},
        tools::ToolDeclarations,
    },
};
use serde_json::Value;
use tools_rs::{CallId, FunctionCall};

use crate::{api::types::request::ClaudeRequest, client::ClaudeClient};

const THINKING_BETA_HEADER: &str = "interleaved-thinking-2025-05-14";

#[async_trait::async_trait]
impl<T: Transport> StreamProvider for ClaudeClient<T> {
    async fn stream(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
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

        let body = serde_json::to_vec(&request_body)
            .map_err(|e| ChatError::InvalidResponse(e.to_string()))?;

        let mut headers = vec![
            ("x-api-key".into(), self.api_key.clone()),
            ("anthropic-version".into(), self.api_version.clone()),
            ("content-type".into(), "application/json".into()),
        ];

        if self.include_thoughts {
            headers.push(("anthropic-beta".into(), THINKING_BETA_HEADER.into()));
        }

        let req = chat_core::transport::Request {
            scheme: self.scheme.clone(),
            host: self.host.clone(),
            path: format!("{}/messages", self.base_path),
            headers,
            body,
        };

        let event_stream = self.transport.stream(req).await.map_err(ChatError::from)?;

        Ok(parse_claude_event_stream(event_stream))
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
                    Ok(Some(PartEnum::Reasoning(
                        Reasoning::new("").with_signature(sig.to_string()),
                    )))
                }
                "input_json_delta" => Ok(None),
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

fn parse_claude_event_stream(
    event_stream: chat_core::transport::EventStream,
) -> BoxStream<'static, Result<StreamEvent, ChatError>> {
    let stream = try_stream! {
        let mut events = event_stream;

        let mut final_parts = Parts::default();
        let mut final_reason = CompleteReasonEnum::None;
        let mut message_id: Option<String> = None;
        let mut model: Option<String> = None;
        let mut input_tokens: usize = 0;
        let mut output_tokens: usize = 0;

        let mut tool_input_buffer = String::new();

        while let Some(event_res) = events.next().await {
            let (event_type, json_str) = event_res.map_err(ChatError::from)?;

            {
                match event_type.as_str() {
                    "message_start" => {
                        if let Ok(data) = serde_json::from_str::<Value>(&json_str)
                            && let Some(msg) = data.get("message") {
                                message_id = msg.get("id").and_then(|v| v.as_str()).map(str::to_string);
                                model = msg.get("model").and_then(|v| v.as_str()).map(str::to_string);
                                if let Some(u) = msg.get("usage") {
                                    input_tokens = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                                }
                            }
                    }
                    "content_block_start" => {
                        if let Ok(data) = serde_json::from_str::<Value>(&json_str) {
                            let block_type = data.get("content_block")
                                .and_then(|c| c.get("type"))
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            if block_type == "tool_use" {
                                tool_input_buffer.clear();
                            }
                        }
                        if let Ok(Some(part)) = sse_event_to_part("content_block_start", &json_str)
                            && let Some(event) = final_parts.merge_chunk(part) {
                                yield event;
                            }
                    }
                    "content_block_delta" => {
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

                        if let Ok(Some(part)) = sse_event_to_part("content_block_delta", &json_str)
                            && let Some(event) = final_parts.merge_chunk(part) {
                                yield event;
                            }
                    }
                    "content_block_stop" => {
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
