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
        metadata::usage::Usage,
        options::ChatOptions,
        response::{ChatResponse, SseParser, StreamEvent},
    },
};

use crate::{
    api::types::{error::handle_openai_error, request::OpenAIRequest, response::OpenAIResponse},
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

                let choice = match oai_resp.choices.pop() {
                    Some(c) => c,
                    None => continue,
                };

                if choice.finish_reason.as_deref().is_some() {
                    let reason = match choice.finish_reason.as_deref() {
                        Some("stop") => CompleteReasonEnum::Stop,
                        Some("length") => CompleteReasonEnum::MaxTokens,
                        Some("tool_calls") => CompleteReasonEnum::Stop,
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

                let chunk_parts = choice.message.into_core_parts(true)?;
                for part in chunk_parts.0 {
                    if let Some(event) = final_parts.merge_chunk(part) {
                        yield event;
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
