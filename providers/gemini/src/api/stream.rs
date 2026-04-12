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
            parts::Parts,
        },
        options::ChatOptions,
        response::{ChatResponse, StreamEvent},
        tools::ToolDeclarations,
    },
};

use crate::{
    api::types::{
        request::GeminiRequest, response::GeminiCompletionResponse,
    },
    client::GeminiClient,
};

#[async_trait::async_trait]
impl<T: Transport> StreamProvider for GeminiClient<T> {
    async fn stream(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError> {
        let path = format!(
            "{}/models/{}:streamGenerateContent?alt=sse",
            self.base_path, self.model_name
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

        let body = serde_json::to_vec(&request_body)
            .map_err(|e| ChatError::InvalidResponse(e.to_string()))?;

        let req = chat_core::transport::Request {
            host: self.host.clone(),
            path,
            headers: vec![
                ("x-goog-api-key".into(), self.api_key.clone()),
                ("Content-Type".into(), "application/json".into()),
            ],
            body,
        };

        let event_stream = self
            .transport
            .stream(req)
            .await
            .map_err(ChatError::from)?;

        Ok(parse_gemini_event_stream(event_stream))
    }
}

fn parse_gemini_event_stream(
    event_stream: chat_core::transport::EventStream,
) -> BoxStream<'static, Result<StreamEvent, ChatError>> {
    let stream = try_stream! {
        let mut events = event_stream;

        let mut final_parts = Parts::default();
        let mut final_reason = CompleteReasonEnum::None;
        let mut final_metadata = None;

        while let Some(event_res) = events.next().await {
            let (_, json_str) = event_res.map_err(ChatError::from)?;

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
