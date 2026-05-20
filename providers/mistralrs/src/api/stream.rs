use async_trait::async_trait;
use chat_core::error::ChatError;
use chat_core::traits::StreamProvider;
use chat_core::types::messages::Messages;
use chat_core::types::messages::content::{Content, RoleEnum};
use chat_core::types::messages::parts::{PartEnum, Parts};
use chat_core::types::messages::text::Text;
use chat_core::types::metadata::Metadata;
use chat_core::types::metadata::usage::Usage as CoreUsage;
use chat_core::types::options::ChatOptions;
use chat_core::types::response::{ChatResponse, StreamEvent};
use chat_core::types::tools::ToolDeclarations;
use futures::StreamExt;
use futures::stream::BoxStream;
use mistralrs::Response as MResponse;

use crate::api::types::request;
use crate::api::types::response::{map_finish_reason, usage_from_m};
use crate::client::MistralRsClient;

#[async_trait]
impl StreamProvider for MistralRsClient {
    async fn stream(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError> {
        let tools_present = tool_declarations.is_some();
        // Build the request synchronously so a malformed Messages surfaces
        // here rather than as a stream item.
        let req = request::from_core(messages, options, None, tools_present)
            .map_err(|f| f.err)?;

        let model = self.model.clone();
        let model_id = self.model_id.clone();

        let s = async_stream::try_stream! {
            // Move `model` into the generator and open the stream from
            // inside. The borrow that `Stream<'_>` holds against the
            // `&Model` is now tied to a value that lives for the
            // lifetime of the generator itself.
            let mut raw = model
                .stream_chat_request(req)
                .await
                .map_err(|e| ChatError::Provider(format!("mistral.rs stream_chat_request: {e}")))?;

            let mut accumulated = String::new();
            let mut finish_reason: Option<String> = None;
            let mut last_usage: Option<CoreUsage> = None;

            while let Some(item) = raw.next().await {
                match item {
                    MResponse::Chunk(chunk) => {
                        if let Some(choice) = chunk.choices.into_iter().next() {
                            if let Some(piece) = choice.delta.content {
                                if !piece.is_empty() {
                                    accumulated.push_str(&piece);
                                    yield StreamEvent::TextChunk(piece);
                                }
                            }
                            if let Some(reason) = choice.finish_reason {
                                finish_reason = Some(reason);
                            }
                        }
                        if let Some(u) = chunk.usage {
                            last_usage = Some(usage_from_m(u));
                        }
                    }
                    MResponse::Done(full) => {
                        if let Some(choice) = full.choices.first() {
                            if finish_reason.is_none() && !choice.finish_reason.is_empty() {
                                finish_reason = Some(choice.finish_reason.clone());
                            }
                            // If the streaming path never emitted text,
                            // fall back to the final message content.
                            if accumulated.is_empty() {
                                if let Some(text) = &choice.message.content {
                                    accumulated = text.clone();
                                    yield StreamEvent::TextChunk(text.clone());
                                }
                            }
                        }
                        last_usage = Some(usage_from_m(full.usage));
                        break;
                    }
                    MResponse::CompletionChunk(_) | MResponse::CompletionDone(_) => {
                        // Legacy completion variants — the chat path shouldn't see these.
                    }
                    MResponse::ModelError(msg, _) | MResponse::CompletionModelError(msg, _) => {
                        Err(ChatError::Provider(format!("mistral.rs model error: {msg}")))?;
                    }
                    MResponse::InternalError(e) | MResponse::ValidationError(e) => {
                        Err(ChatError::Provider(format!("mistral.rs internal error: {e}")))?;
                    }
                    _ => {}
                }
            }

            let response = ChatResponse {
                metadata: Some(Metadata {
                    model_slug: Some(model_id.clone()),
                    usage: last_usage.unwrap_or_default(),
                    ..Default::default()
                }),
                content: Content {
                    role: RoleEnum::Model,
                    parts: Parts(vec![PartEnum::Text(Text::new(accumulated))]),
                    complete_reason: map_finish_reason(finish_reason.as_deref()),
                },
            };
            yield StreamEvent::Done(response);
        };

        Ok(s.boxed())
    }
}
