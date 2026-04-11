use async_stream::try_stream;
use futures::{StreamExt, stream::BoxStream};

use chat_core::{
    error::ChatError,
    traits::StreamProvider,
    types::{
        messages::{
            Messages,
            content::{CompleteReasonEnum, Content, RoleEnum},
            parts::Parts,
        },
        options::ChatOptions,
        response::{ChatResponse, SseParser, StreamEvent},
        tools::ToolDeclarations,
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
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:streamGenerateContent?alt=sse",
            self.model_name
        );

        let request_body = GeminiRequest::from_core(
            messages,
            tool_declarations,
            Some(self.native_tools.as_slice()),
            self.function_config.as_ref(),
            options,
            None,
            self.include_thoughts,
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

        Ok(parse_gemini_sse_stream(res))
    }
}

fn parse_gemini_sse_stream(
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

            while let Some((_, json_str)) = sse_parser.next_event() {
                let gemini_chunk = serde_json::from_str::<GeminiCompletionResponse>(&json_str)
                    .map_err(|e| {
                        ChatError::InvalidResponse(format!("Failed to parse Gemini SSE chunk: {e}"))
                    })?;
                let core_resp = gemini_chunk.into_core_chat_response()?;
                if core_resp.content.complete_reason != CompleteReasonEnum::None {
                    final_reason = core_resp.content.complete_reason;
                }
                if core_resp.metadata.is_some() {
                    final_metadata = core_resp.metadata;
                }
                for part in core_resp.content.parts.0 {
                    if let Some(event) = final_parts.merge_chunk(part) {
                        yield event;
                    }
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
