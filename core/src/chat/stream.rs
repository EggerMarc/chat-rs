use async_stream::try_stream;
use futures::{StreamExt, stream::BoxStream};

use crate::{
    chat::{Chat, state::Unstructured},
    error::ChatFailure,
    traits::StreamProvider,
    types::{
        messages::{Messages, parts::PartEnum},
        metadata::Metadata,
        response::{ChatResponse, StreamEvent},
    },
};

impl<CP: StreamProvider> Chat<CP, Unstructured> {
    /// Streaming chat loop with HITL support.
    ///
    /// Yields each token/chunk as `StreamEvent::TextChunk` / similar. When
    /// a tool strategy pauses execution (for example, `RequireApproval`),
    /// the stream yields `StreamEvent::Paused(PauseReason)` and then
    /// terminates. The caller resolves pending tools on `messages` —
    /// typically via `Messages::find_tool_mut` — and calls `stream()`
    /// again to continue. On re-entry, a pre-step executes any
    /// newly-approved tools, emits `ToolResult` events for them, and
    /// then falls through into the next provider turn.
    pub async fn stream<'a>(
        &'a mut self,
        messages: &'a mut Messages,
    ) -> Result<BoxStream<'a, Result<StreamEvent, ChatFailure>>, ChatFailure> {
        if let Some(strategy) = self.before_strategy.as_mut() {
            strategy(messages, None).await;
        }

        let stream = try_stream! {
            let max_steps = self.max_steps.unwrap_or(1);
            let mut last_metadata: Option<Metadata> = None;

            for _ in 0..max_steps {
                // Pre-step: execute any tools already resolved to
                // Approved on the last Content (typically from a
                // prior pause that the caller just resolved). Emit
                // ToolResult events for completed tools. Yield Paused
                // if the pre-step itself produced a pause (can happen
                // if the caller left some tools still Pending).
                if let Some(last) = messages.0.last_mut() {
                    let pass = self
                        .tool_call(last)
                        .await
                        .map_err(|err| ChatFailure {
                            err,
                            metadata: last_metadata.clone(),
                        })?;

                    if pass.executed
                        && let Some(last) = messages.0.last()
                    {
                        for tool in last.parts.tools() {
                            if let Some(fr) = tool.response() {
                                yield StreamEvent::ToolResult(fr.clone());
                            }
                        }
                    }

                    if let Some(reason) = pass.pause {
                        yield StreamEvent::Paused(reason);
                        return;
                    }
                }

                let decls =
                    crate::chat::tool_declarations_from(&self.scoped_collections);
                let decls_dyn = decls
                    .as_ref()
                    .map(|d| d as &dyn crate::types::tools::ToolDeclarations);
                let mut provider_stream = self
                    .model
                    .stream(messages, decls_dyn, self.model_options.as_ref())
                    .await
                    .map_err(|err| ChatFailure { err, metadata: last_metadata.clone() })?;

                let mut final_response: Option<ChatResponse> = None;
                // Mid-stream Structured events are also accumulated into
                // the final ChatResponse so non-streaming consumers see
                // them in `content.parts`, preserving the equivalence
                // between `complete()` and accumulated `stream()`.
                let mut structured_buffer: Vec<serde_json::Value> = Vec::new();

                while let Some(event_result) = provider_stream.next().await {
                    match event_result {
                        Ok(StreamEvent::Done(response)) => {
                            final_response = Some(response);
                        }
                        Ok(event) => {
                            if let StreamEvent::Structured(ref v) = event {
                                structured_buffer.push(v.clone());
                            }
                            yield event;
                        }
                        Err(err) => {
                            Err(ChatFailure { err, metadata: last_metadata.clone() })?;
                        }
                    }
                }

                if let Some(mut response) = final_response {
                    for v in structured_buffer.drain(..) {
                        response.content.parts.push(PartEnum::Structured(v));
                    }
                    self.model.on_stream_done(&response);

                    if let Some(metadata) = response.metadata.clone() {
                        match &mut last_metadata {
                            Some(existing) => { existing.extend(&metadata); },
                            None => { last_metadata = Some(metadata); },
                        }
                    }

                    messages.push(response.content.clone());

                    // Post-step: apply strategy to any tools the model
                    // emitted this turn. Execute those that say Execute;
                    // pause on anything that needs approval/deferral.
                    let pass = match messages.0.last_mut() {
                        Some(last) => self.tool_call(last).await
                            .map_err(|err| ChatFailure { err, metadata: last_metadata.clone() })?,
                        None => crate::chat::ToolCallPass::default(),
                    };

                    if pass.executed
                        && let Some(last) = messages.0.last()
                    {
                        for tool in last.parts.tools() {
                            if let Some(fr) = tool.response() {
                                yield StreamEvent::ToolResult(fr.clone());
                            }
                        }
                    }

                    if let Some(reason) = pass.pause {
                        yield StreamEvent::Paused(reason);
                        return;
                    }

                    if pass.executed {
                        // Tools ran; need another provider turn so the
                        // model can react to the results.
                        continue;
                    }

                    if let Some(strategy) = self.after_strategy.as_mut() {
                        strategy(messages, last_metadata.as_ref()).await;
                    }
                    yield StreamEvent::Done(response);
                    break;
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::ChatError,
        types::{
            messages::{
                Messages,
                content::{Content, RoleEnum},
                parts::{PartEnum, Parts},
            },
            options::ChatOptions,
            response::ChatResponse,
            tools::ToolDeclarations,
        },
    };
    use async_trait::async_trait;
    use serde_json::json;
    use std::collections::HashMap;
    use std::marker::PhantomData;

    /// Minimal `StreamProvider` that yields a pre-loaded event sequence,
    /// then ends. Used to exercise the engine's accumulator without
    /// touching a real network or wire protocol.
    struct MockStreamProvider {
        events: Vec<Result<StreamEvent, ChatError>>,
    }

    #[async_trait]
    impl StreamProvider for MockStreamProvider {
        async fn stream(
            &mut self,
            _messages: &mut Messages,
            _tool_declarations: Option<&dyn ToolDeclarations>,
            _options: Option<&ChatOptions>,
        ) -> Result<
            futures::stream::BoxStream<'static, Result<StreamEvent, ChatError>>,
            ChatError,
        > {
            let events = std::mem::take(&mut self.events);
            Ok(Box::pin(futures::stream::iter(events)))
        }
    }

    fn chat_with(
        events: Vec<Result<StreamEvent, ChatError>>,
    ) -> Chat<MockStreamProvider, Unstructured> {
        Chat {
            model: MockStreamProvider { events },
            output_shape: None,
            model_options: None,
            max_steps: Some(1),
            max_retries: None,
            retry_strategy: None,
            before_strategy: None,
            after_strategy: None,
            scoped_collections: Vec::new(),
            routing: HashMap::new(),
            _output: PhantomData,
        }
    }

    /// Helper: builds an empty `Done` response — the provider's "I'm done"
    /// signal. The contract we're testing is that mid-stream `Structured`
    /// events get folded into `content.parts` by the engine, not the
    /// provider, so the provider's `Done` has no parts of its own.
    fn done_event() -> StreamEvent {
        StreamEvent::Done(ChatResponse {
            content: Content {
                role: RoleEnum::Model,
                parts: Parts::default(),
                complete_reason: Default::default(),
            },
            metadata: None,
        })
    }

    async fn collect_stream(
        chat: &mut Chat<MockStreamProvider, Unstructured>,
        messages: &mut Messages,
    ) -> Vec<StreamEvent> {
        let mut s = chat.stream(messages).await.expect("stream open");
        let mut out = Vec::new();
        while let Some(ev) = s.next().await {
            out.push(ev.expect("event ok"));
        }
        out
    }

    #[tokio::test]
    async fn structured_events_flow_to_consumer_and_into_final_response() {
        let mut chat = chat_with(vec![
            Ok(StreamEvent::Structured(json!({"step": 1}))),
            Ok(StreamEvent::Structured(json!({"step": 2}))),
            Ok(done_event()),
        ]);
        let mut messages = Messages::default();

        let events = collect_stream(&mut chat, &mut messages).await;

        // Consumer sees: 2 Structured events + the final Done.
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], StreamEvent::Structured(_)));
        assert!(matches!(events[1], StreamEvent::Structured(_)));

        // Final Done carries a ChatResponse whose parts include both
        // Structured values, in order.
        let StreamEvent::Done(response) = &events[2] else {
            panic!("expected Done event");
        };
        let structured: Vec<&serde_json::Value> = response
            .content
            .parts
            .0
            .iter()
            .filter_map(|p| match p {
                PartEnum::Structured(v) => Some(v),
                _ => None,
            })
            .collect();
        assert_eq!(structured.len(), 2);
        assert_eq!(structured[0], &json!({"step": 1}));
        assert_eq!(structured[1], &json!({"step": 2}));
    }

    #[tokio::test]
    async fn structured_interleaved_with_text_preserves_event_order() {
        let mut chat = chat_with(vec![
            Ok(StreamEvent::TextChunk("hello ".into())),
            Ok(StreamEvent::Structured(json!({"step": 1}))),
            Ok(StreamEvent::TextChunk("world".into())),
            Ok(StreamEvent::Structured(json!({"step": 2}))),
            Ok(done_event()),
        ]);
        let mut messages = Messages::default();

        let events = collect_stream(&mut chat, &mut messages).await;

        // Event order on the consumer side is exactly what the provider
        // emitted, untouched by the accumulator.
        assert_eq!(events.len(), 5);
        assert!(matches!(events[0], StreamEvent::TextChunk(ref t) if t == "hello "));
        assert!(matches!(events[1], StreamEvent::Structured(_)));
        assert!(matches!(events[2], StreamEvent::TextChunk(ref t) if t == "world"));
        assert!(matches!(events[3], StreamEvent::Structured(_)));

        // Final response.parts contains only the Structured entries
        // (no text — provider's Done was empty), in order.
        let StreamEvent::Done(response) = &events[4] else {
            panic!("expected Done event");
        };
        let parts: Vec<&PartEnum> = response.content.parts.0.iter().collect();
        assert_eq!(parts.len(), 2);
        assert!(matches!(parts[0], PartEnum::Structured(v) if v == &json!({"step": 1})));
        assert!(matches!(parts[1], PartEnum::Structured(v) if v == &json!({"step": 2})));
    }

    #[tokio::test]
    async fn no_structured_events_leaves_final_response_untouched() {
        // Regression guard: the buffer-and-drain path must not corrupt
        // the response when no Structured events appear.
        let mut chat = chat_with(vec![
            Ok(StreamEvent::TextChunk("just text".into())),
            Ok(done_event()),
        ]);
        let mut messages = Messages::default();

        let events = collect_stream(&mut chat, &mut messages).await;

        assert_eq!(events.len(), 2);
        let StreamEvent::Done(response) = &events[1] else {
            panic!("expected Done event");
        };
        assert!(response.content.parts.0.is_empty());
    }
}
