use async_trait::async_trait;
use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::CompletionProvider;
use chat_core::types::messages::Messages;
use chat_core::types::options::ChatOptions;
use chat_core::types::provider_meta::ProviderMeta;
use chat_core::types::response::ChatResponse;
use chat_core::types::tools::ToolDeclarations;

use crate::api::types::request::{self, TurnPlan};
use crate::api::types::response;
use crate::client::AppleFMClient;
use crate::ffi;

#[async_trait]
impl CompletionProvider for AppleFMClient {
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        let (instructions, convo) =
            request::prepare(messages, structured_output, tool_declarations.is_some())?;
        let wire_options = request::merge_options(&self.config, options);
        let instructions_hash = request::hash_instructions(instructions.as_deref());

        // Held for the whole turn: serializes use of the bridge session.
        let mut session = self.session.lock().await;

        let reused = match session.plan(instructions_hash, &convo) {
            TurnPlan::Reuse => true,
            TurnPlan::Rebuild => {
                session.invalidate();
                let config_json =
                    request::session_config_json(instructions.as_deref(), &self.config)?;
                let created_json =
                    tokio::task::spawn_blocking(move || ffi::session_create(&config_json))
                        .await
                        .map_err(join_error)?;
                session.install(
                    response::parse_session_created(&created_json)?,
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
        let turn_json = request::turn_request_json(message, wire_options)?;
        let session_id = session.id().expect("session installed above");

        // The bridge call blocks (model inference); keep it off the
        // async workers.
        let started = std::time::Instant::now();
        let reply_json =
            tokio::task::spawn_blocking(move || ffi::session_respond(session_id, &turn_json))
                .await
                .map_err(join_error)?;

        match response::into_core(&self.model_slug(), &reply_json) {
            Ok(mut chat_response) => {
                // Advance the fingerprint past this turn (the chat loop
                // appends the reply to `Messages`, so the next call sees
                // convo + reply + new user message).
                let reply_text = chat_response
                    .content
                    .parts
                    .text_response()
                    .map(|t| t.as_str().to_owned())
                    .unwrap_or_default();
                session.advance(convo, reply_text);

                if let Some(metadata) = chat_response.metadata.as_mut() {
                    self.enrich_metadata(metadata, started.elapsed(), reused);
                }
                Ok(chat_response)
            }
            Err(failure) => {
                // The bridge session may hold a half-applied turn.
                session.invalidate();
                Err(failure)
            }
        }
    }

    fn metadata(&self) -> Option<&ProviderMeta> {
        Some(&self.meta)
    }
}

fn join_error(e: tokio::task::JoinError) -> ChatFailure {
    ChatFailure::from_err(ChatError::Other(format!("bridge task failed: {e}")))
}
