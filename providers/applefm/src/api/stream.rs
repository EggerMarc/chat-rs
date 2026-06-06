use async_trait::async_trait;
use chat_core::error::ChatError;
use chat_core::traits::StreamProvider;
use chat_core::types::messages::Messages;
use chat_core::types::messages::content::{Content, RoleEnum};
use chat_core::types::messages::parts::{PartEnum, Parts};
use chat_core::types::messages::text::Text;
use chat_core::types::metadata::Metadata;
use chat_core::types::options::ChatOptions;
use chat_core::types::response::{ChatResponse, StreamEvent};
use chat_core::types::tools::ToolDeclarations;
use futures::StreamExt;
use futures::stream::BoxStream;

use crate::api::types::{WireStreamEvent, request, response};
use crate::client::AppleFMClient;
use crate::ffi;

#[async_trait]
impl StreamProvider for AppleFMClient {
    async fn stream(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError> {
        let request_json = request::from_core(
            &self.config,
            messages,
            options,
            None,
            tool_declarations.is_some(),
        )
        .map_err(|failure| failure.err)?;
        let model_slug = self.model_slug();

        // The bridge call blocks until the stream finishes, emitting one
        // JSON event per callback; pump events through a channel into the
        // async world.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        tokio::task::spawn_blocking(move || {
            ffi::stream_json(&request_json, |event| {
                let _ = tx.send(event.to_owned());
            });
        });

        let stream = async_stream::try_stream! {
            while let Some(event_json) = rx.recv().await {
                let event: WireStreamEvent =
                    serde_json::from_str(&event_json).map_err(|e| {
                        ChatError::InvalidResponse(format!(
                            "malformed bridge stream event ({e}): {event_json}"
                        ))
                    })?;
                match event {
                    WireStreamEvent::Delta { text } => {
                        yield StreamEvent::TextChunk(text);
                    }
                    WireStreamEvent::Done { text, finish } => {
                        // The done event carries the authoritative full
                        // text; deltas are best-effort display fragments.
                        yield StreamEvent::Done(ChatResponse {
                            metadata: Some(Metadata {
                                model_slug: Some(model_slug.clone()),
                                ..Default::default()
                            }),
                            content: Content {
                                role: RoleEnum::Model,
                                parts: Parts(vec![PartEnum::Text(Text::new(text))]),
                                complete_reason: response::map_finish(&finish),
                            },
                        });
                        break;
                    }
                    WireStreamEvent::Error { error } => {
                        Err(response::error_to_chat(error))?;
                    }
                }
            }
        };

        Ok(stream.boxed())
    }
}
