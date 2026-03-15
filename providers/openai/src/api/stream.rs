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
            parts::Parts,
        },
        options::ChatOptions,
        response::{ChatResponse, SseParser, StreamEvent},
    },
};

use crate::{
    api::types::{
        error::handle_openai_error,
        request::OpenAIRequest,
        response::OpenAIStreamChunk,
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

        while let Some(chunk_res) = byte_stream.next().await {
            let chunk = chunk_res.map_err(|e| ChatError::Network(e.to_string()))?;

            sse_parser.push(&chunk);

            while let Some(json_str) = sse_parser.next_event() {
                // OpenAI signals end-of-stream with [DONE]
                if json_str.trim() == "[DONE]" {
                    continue;
                }

                let oai_chunk = serde_json::from_str::<OpenAIStreamChunk>(&json_str)
                    .map_err(|e| {
                        ChatError::InvalidResponse(format!("Failed to parse OpenAI SSE chunk: {e}"))
                    })?;
                let core_resp = oai_chunk.into_core_chat_response()?;
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
