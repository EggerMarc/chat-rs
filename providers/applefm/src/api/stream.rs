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

use crate::api::types::request::{self, TurnPlan};
use crate::api::types::{WireStreamEvent, response};
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
        let (instructions, convo) = request::prepare(messages, None, tool_declarations.is_some())
            .map_err(|failure| failure.err)?;
        let wire_options = request::merge_options(&self.config, options);
        let instructions_hash = request::hash_instructions(instructions.as_deref());

        // Owned guard: held until the stream finishes (it moves into the
        // generator), serializing use of the bridge session.
        let mut session = self.session.clone().lock_owned().await;

        let reused = match session.plan(instructions_hash, &convo) {
            TurnPlan::Reuse => true,
            TurnPlan::Rebuild => {
                session.invalidate();
                let config_json =
                    request::session_config_json(instructions.as_deref(), &self.config)
                        .map_err(|failure| failure.err)?;
                let created_json =
                    tokio::task::spawn_blocking(move || ffi::session_create(&config_json))
                        .await
                        .map_err(|e| ChatError::Other(format!("bridge task failed: {e}")))?;
                session.install(
                    response::parse_session_created(&created_json)
                        .map_err(|failure| failure.err)?,
                    instructions_hash,
                );
                false
            }
        };

        let message = if reused {
            convo
                .last()
                .expect("prepare guarantees non-empty")
                .text
                .clone()
        } else {
            request::render_full(&convo)
        };
        let turn_json =
            request::turn_request_json(message, wire_options).map_err(|failure| failure.err)?;
        let session_id = session.id().expect("session installed above");

        let client = self.clone();
        let started = std::time::Instant::now();

        // The bridge call blocks until the stream finishes, emitting one
        // JSON event per callback; pump events through a channel into the
        // async world.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
        tokio::task::spawn_blocking(move || {
            ffi::session_stream(session_id, &turn_json, |event| {
                let _ = tx.send(event.to_owned());
            });
        });

        let stream = async_stream::try_stream! {
            // If anything below errors (or the consumer drops us before
            // `Done` advanced the fingerprint), the next turn replans as
            // a rebuild; on explicit errors we also invalidate so the
            // half-applied bridge session is released.
            let mut session = session;
            let mut convo = Some(convo);

            while let Some(event_json) = rx.recv().await {
                let event: WireStreamEvent = match serde_json::from_str(&event_json) {
                    Ok(event) => event,
                    Err(e) => {
                        session.invalidate();
                        Err(ChatError::InvalidResponse(format!(
                            "malformed bridge stream event ({e}): {event_json}"
                        )))?;
                        unreachable!()
                    }
                };
                match event {
                    WireStreamEvent::Delta { text } => {
                        yield StreamEvent::TextChunk(text);
                    }
                    WireStreamEvent::Done { text, finish } => {
                        // The done event carries the authoritative full
                        // text; deltas are best-effort display fragments.
                        if let Some(convo) = convo.take() {
                            session.advance(convo, text.clone());
                        }
                        let mut metadata = Metadata {
                            model_slug: Some(client.model_slug()),
                            ..Default::default()
                        };
                        client.enrich_metadata(&mut metadata, started.elapsed(), reused);
                        yield StreamEvent::Done(ChatResponse {
                            metadata: Some(metadata),
                            content: Content {
                                role: RoleEnum::Model,
                                parts: Parts(vec![PartEnum::Text(Text::new(text))]),
                                complete_reason: response::map_finish(&finish),
                            },
                        });
                        break;
                    }
                    WireStreamEvent::Error { error } => {
                        session.invalidate();
                        Err(response::error_to_chat(error))?;
                    }
                }
            }
        };

        Ok(stream.boxed())
    }
}
