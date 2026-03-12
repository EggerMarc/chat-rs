use async_stream::try_stream;
use futures::{StreamExt, stream::BoxStream};
use tools_rs::ToolCollection;

use chat_core::{
    error::ChatError,
    traits::StreamProvider,
    types::{
        messages::{
            Messages,
            content::{CompleteReasonEnum, Content, RoleEnum},
            parts::{PartEnum, Parts},
        },
        options::ChatOptions,
        response::{ChatResponse, StreamEvent},
    },
};

use crate::{
    api::types::{
        error::handle_gemini_error, request::GeminiRequest, response::GeminiCompletionResponse,
    },
    client::GeminiClient,
};

#[async_trait::async_trait]
impl StreamProvider for GeminiClient {
    async fn stream(
        &self,
        messages: &mut Messages,
        tools: Option<&ToolCollection>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse",
            self.model_name
        );

        let request_body = GeminiRequest::from_core(
            messages,
            tools,
            Some(self.native_tools.as_slice()),
            self.function_config.as_ref(),
            options,
            None, // Structured output is unsupported in streaming
        )?;

        let res = self
            .http_client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ChatError::Network(e.to_string()))?;

        // Catch 400/500 errors immediately
        let res = handle_gemini_error(res)
            .await
            .map_err(|failure| failure.err)?;

        let stream = try_stream! {
            let mut byte_stream = res.bytes_stream();
            let mut string_buffer = String::new();

            let mut final_parts = Parts::default();
            let mut final_reason = CompleteReasonEnum::None;
            let mut final_metadata = None;

            // Read raw bytes as they arrive
            while let Some(chunk_res) = byte_stream.next().await {
                let chunk = chunk_res.map_err(|e| ChatError::Network(e.to_string()))?;
                string_buffer.push_str(&String::from_utf8_lossy(&chunk));

                // Process complete lines separated by \n
                while let Some(newline_pos) = string_buffer.find('\n') {
                    let line = string_buffer[..newline_pos].trim().to_string();
                    string_buffer.drain(..newline_pos + 1);

                    if line.is_empty() || !line.starts_with("data: ") {
                        continue;
                    }

                    let json_str = line.strip_prefix("data: ").unwrap().trim();
                    if json_str == "[DONE]" {
                        continue;
                    }

                    // Explicitly catch JSON parsing errors
                    match serde_json::from_str::<GeminiCompletionResponse>(json_str) {
                        Ok(gemini_chunk) => {
                            if let Ok(core_resp) = gemini_chunk.into_core_chat_response() {

                                if core_resp.content.complete_reason != CompleteReasonEnum::None {
                                    final_reason = core_resp.content.complete_reason;
                                }
                                if core_resp.metadata.is_some() {
                                    final_metadata = core_resp.metadata;
                                }

                                // Intelligently merge the parts into our final history
                                for part in core_resp.content.parts.0 {
                                    match part {
                                        PartEnum::Reasoning(mut new_r) => {
                                            yield StreamEvent::ReasoningChunk(new_r.text.clone());

                                            if let Some(PartEnum::Reasoning(last_r)) = final_parts.0.last_mut() {
                                                last_r.text.push_str(&new_r.text);
                                                if last_r.signature.is_none() && new_r.signature.is_some() {
                                                    last_r.signature = new_r.signature;
                                                }
                                            } else {
                                                final_parts.push(PartEnum::Reasoning(new_r));
                                            }
                                        }
                                        PartEnum::Text(new_t) => {
                                            yield StreamEvent::TextChunk(new_t.0.clone());

                                            if let Some(PartEnum::Text(last_t)) = final_parts.0.last_mut() {
                                                last_t.0.push_str(&new_t.0);
                                            } else {
                                                final_parts.push(PartEnum::Text(new_t));
                                            }
                                        }
                                        PartEnum::FunctionCall(new_fc) => {
                                            // Deduplicate: Google sends partial arguments across chunks
                                            if let Some(PartEnum::FunctionCall(last_fc)) = final_parts.0.last_mut() {
                                                if last_fc.name == new_fc.name {
                                                    last_fc.arguments = new_fc.arguments.clone();
                                                    if last_fc.id.is_none() && new_fc.id.is_some() {
                                                        last_fc.id = new_fc.id.clone();
                                                    }
                                                } else {
                                                    final_parts.push(PartEnum::FunctionCall(new_fc.clone()));
                                                    yield StreamEvent::ToolCall(new_fc);
                                                }
                                            } else {
                                                // First time seeing this tool
                                                final_parts.push(PartEnum::FunctionCall(new_fc.clone()));
                                                yield StreamEvent::ToolCall(new_fc);
                                            }
                                        }
                                        _ => {} // Catch-all for unrecognized parts
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            // Safe fallback: Prevents the stream from silently breaking
                            eprintln!("Failed to parse SSE JSON chunk: {}\nRaw String: {}", e, json_str);
                        }
                    }
                }
            }

            // The stream has finished. Yield the perfectly assembled ChatResponse!
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

        Ok(Box::pin(stream))
    }
}
