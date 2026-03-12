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
            text::Text,
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
            None,
        )?;

        let res = self
            .http_client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ChatError::Network(e.to_string()))?;

        let res = handle_gemini_error(res)
            .await
            .map_err(|failure| failure.err)?;

        let stream = try_stream! {
            let mut byte_stream = res.bytes_stream();
            let mut string_buffer = String::new();
            let mut full_text = String::new();
            let mut final_parts = Parts::default();
            let mut final_reason = CompleteReasonEnum::None;
            let mut final_metadata = None;

            // Read raw bytes as they arrive
            while let Some(chunk_res) = byte_stream.next().await {
                let chunk = chunk_res.map_err(|e| ChatError::Network(e.to_string()))?;
                string_buffer.push_str(&String::from_utf8_lossy(&chunk));

                // Process complete lines
                while let Some(newline_pos) = string_buffer.find('\n') {
                    let line = string_buffer[..newline_pos].trim().to_string();
                    string_buffer.drain(..newline_pos + 1);

                    // Skip empty lines or non-data lines
                    if line.is_empty() || !line.starts_with("data: ") {
                        continue;
                    }

                    let json_str = line.strip_prefix("data: ").unwrap().trim();
                    if json_str == "[DONE]" {
                        continue;
                    }

                    // Parse the chunk into our existing Completion struct
                    if let Ok(gemini_chunk) = serde_json::from_str::<GeminiCompletionResponse>(json_str) {
                        // Convert to core response using our existing helper!
                        if let Ok(core_resp) = gemini_chunk.into_core_chat_response() {

                            // Capture finish reasons and metadata (usually sent in the last chunk)
                            if core_resp.content.complete_reason != CompleteReasonEnum::None {
                                final_reason = core_resp.content.complete_reason;
                            }
                            if core_resp.metadata.is_some() {
                                final_metadata = core_resp.metadata;
                            }

                            let mut chunk_text = String::new();

                            // Dissect the parts
                            for part in core_resp.content.parts.0 {
                                match part {
                                    PartEnum::Reasoning(t) => {
                                        chunk_text.push_str(&t.0);
                                    },
                                    PartEnum::Text(t) => {
                                        chunk_text.push_str(&t.0);
                                    }
                                    PartEnum::FunctionCall(fc) => {
                                        // Accumulate function calls (we don't yield them to the user)
                                        final_parts.push(PartEnum::FunctionCall(fc));
                                    }
                                    _ => {}
                                }
                            }

                            // If text was generated, yield it to the user instantly!
                            if !chunk_text.is_empty() {
                                full_text.push_str(&chunk_text);
                                yield StreamEvent::TextChunk(chunk_text);
                            }
                        }
                    }
                }
            }

            // The stream has finished downloading. Assemble the final response!
            if !full_text.is_empty() {
                final_parts.0.insert(0, PartEnum::Text(Text::new(&full_text)));
            }

            let final_response = ChatResponse {
                content: Content {
                    role: RoleEnum::Model,
                    parts: final_parts,
                    complete_reason: final_reason,
                },
                metadata: final_metadata,
            };

            // Yield the final assembled payload so the engine can save it to history
            yield StreamEvent::Done(final_response);
        };

        Ok(Box::pin(stream))
    }
}
