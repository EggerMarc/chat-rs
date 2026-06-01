//! Streaming chat loops, feature-gated as a unit.
//!
//! This module root *is* the base streaming library: the one shared engine
//! ([`Chat::run_stream`]) that drives **both** streaming surfaces, the
//! output-only `Chat<CP, Unstructured>::stream` entry point, and the pieces
//! every loop shares — the pre-step tool pass ([`Chat::stream_tool_pass`]), the
//! response commit + post-step tool pass ([`Chat::stream_commit`]), and
//! per-event classification ([`pump_event`]).
//!
//! - [`handle`] — the bidirectional handles (`ChatStream`, `InputStream`,
//!   `OutputStream`, `Input`, `IntoInput`, `SendError`).
//! - [`input`] — the input-specific delta: the `Chat<CP, InputStreamed>::stream`
//!   entry point plus the merge helpers the engine calls when input is present.
//!
//! The engine is parameterised by an optional input channel: `None` drains the
//! provider directly (vanilla), `Some(rx)` races it against caller input and
//! restarts on each burst. *Everything else* — steps, tool passes, metadata,
//! structured accumulation, event handling — is one body, written once.

pub mod handle;
mod input;

pub use handle::{ChatStream, Input, InputStream, IntoInput, OutputStream, SendError};

use async_stream::try_stream;
use futures::{Stream, StreamExt, channel::mpsc, future::Either, stream::BoxStream};
use tools_rs::FunctionResponse;

use input::{InputSignal, apply_input_to_messages, next_input};

use crate::{
    chat::{Chat, ToolCallPass, state::Unstructured},
    error::{ChatError, ChatFailure},
    traits::StreamProvider,
    types::{
        messages::{Messages, parts::PartEnum},
        metadata::Metadata,
        response::{ChatResponse, StreamEvent},
    },
};

/// One classified provider-stream event.
///
/// `Done` ends the turn (its `ChatResponse` becomes the step's result); `Yield`
/// is forwarded to the consumer. This is the single home for the per-event
/// logic — Structured buffering and Done detection — so the engine's two
/// acquisition paths (race-input vs. direct) don't each reimplement it.
pub(crate) enum Pump {
    Yield(StreamEvent),
    Done(ChatResponse),
}

/// Classify one item off a provider stream: surface errors, buffer
/// `Structured` values for the final response, and tell the loop whether to
/// yield the event or treat it as the turn's end.
pub(crate) fn pump_event(
    item: Result<StreamEvent, ChatError>,
    structured_buffer: &mut Vec<serde_json::Value>,
) -> Result<Pump, ChatError> {
    match item {
        Ok(StreamEvent::Done(response)) => Ok(Pump::Done(response)),
        Ok(event) => {
            // Mid-stream Structured events are accumulated into the final
            // ChatResponse so non-streaming consumers see them in
            // `content.parts`, preserving `complete()` ⇄ `stream()` equivalence.
            if let StreamEvent::Structured(ref v) = event {
                structured_buffer.push(v.clone());
            }
            Ok(Pump::Yield(event))
        }
        Err(err) => Err(err),
    }
}

impl<CP: StreamProvider, Output> Chat<CP, Output> {
    /// Pre-step: run the tool pass on the last content and collect the
    /// `ToolResult` responses the loop should emit, plus the executed/pause
    /// signal. Shared by every step.
    pub(crate) async fn stream_tool_pass(
        &self,
        messages: &mut Messages,
        last_metadata: &Option<Metadata>,
    ) -> Result<(Vec<FunctionResponse>, ToolCallPass), ChatFailure> {
        let pass = match messages.0.last_mut() {
            Some(last) => self.tool_call(last).await.map_err(|err| ChatFailure {
                err,
                metadata: last_metadata.clone(),
            })?,
            None => ToolCallPass::default(),
        };
        Ok((collect_tool_results(&pass, messages), pass))
    }

    /// Post-step: fire `on_stream_done`, fold metadata, commit the response
    /// into `Messages`, and run the tool pass on it. Returns the `ToolResult`
    /// responses to emit and the signal.
    ///
    /// The caller is expected to have already folded any accumulated
    /// `StreamEvent::Structured` values into `response.content.parts`.
    pub(crate) async fn stream_commit(
        &mut self,
        messages: &mut Messages,
        response: &ChatResponse,
        last_metadata: &mut Option<Metadata>,
    ) -> Result<(Vec<FunctionResponse>, ToolCallPass), ChatFailure> {
        self.model.on_stream_done(response);

        if let Some(metadata) = response.metadata.clone() {
            match last_metadata {
                Some(existing) => {
                    existing.extend(&metadata);
                }
                None => {
                    *last_metadata = Some(metadata);
                }
            }
        }

        messages.push(response.content.clone());

        let pass = match messages.0.last_mut() {
            Some(last) => self.tool_call(last).await.map_err(|err| ChatFailure {
                err,
                metadata: last_metadata.clone(),
            })?,
            None => ToolCallPass::default(),
        };
        Ok((collect_tool_results(&pass, messages), pass))
    }

    /// The shared streaming engine — one loop for both `Chat<Unstructured>` and
    /// `Chat<InputStreamed>`.
    ///
    /// `input` is the *only* difference between the two surfaces: `Some(rx)`
    /// races the provider against caller-pushed input and restarts the provider
    /// on each burst; `None` simply drains the provider. Steps, tool passes,
    /// metadata folding, structured accumulation, and per-event handling are
    /// identical — written here once.
    pub(crate) fn run_stream<'a>(
        &'a mut self,
        messages: &'a mut Messages,
        mut input: Option<mpsc::UnboundedReceiver<Input>>,
    ) -> impl Stream<Item = Result<StreamEvent, ChatFailure>> + Send + 'a
    where
        Output: Send + Sync,
    {
        try_stream! {
            let max_steps = self.max_steps.unwrap_or(1);
            let mut last_metadata: Option<Metadata> = None;
            // Goes false permanently once all producers drop; for `None` input
            // it starts false, so the race path below is never taken.
            let mut input_open = input.is_some();

            'step: for _ in 0..max_steps {
                // Pre-step: drain already-resolved/approved tools, emit their
                // ToolResults, pause if the caller still has work to do.
                let (results, pass) = self.stream_tool_pass(messages, &last_metadata).await?;
                for fr in results {
                    yield StreamEvent::ToolResult(fr);
                }
                if let Some(reason) = pass.pause {
                    yield StreamEvent::Paused(reason);
                    return;
                }

                let decls = crate::chat::tool_declarations_from(&self.scoped_collections);
                let decls_dyn = decls
                    .as_ref()
                    .map(|d| d as &dyn crate::types::tools::ToolDeclarations);

                // Restart loop: with input, a burst drops the current provider
                // stream and re-enters with mutated Messages. Without input it
                // runs exactly once.
                'restart: loop {
                    let mut provider_stream = self
                        .model
                        .stream(messages, decls_dyn, self.model_options.as_ref())
                        .await
                        .map_err(|err| ChatFailure { err, metadata: last_metadata.clone() })?;

                    let mut final_response: Option<ChatResponse> = None;
                    let mut structured_buffer: Vec<serde_json::Value> = Vec::new();

                    loop {
                        // Acquire the next provider item. With an open input
                        // channel, race it against input (restart / cancel /
                        // close accordingly); otherwise poll the provider
                        // directly. Both paths feed the same `pump_event`.
                        let item = match input.as_mut().filter(|_| input_open) {
                            Some(rx) => {
                                // The input future borrows `rx` (which lives
                                // outside it), so losing the race and being
                                // dropped is cancel-safe — no input is lost.
                                let provider_next = provider_stream.next();
                                let input_next = next_input(rx);
                                match futures::future::select(
                                    Box::pin(provider_next),
                                    Box::pin(input_next),
                                )
                                .await
                                {
                                    Either::Left((item, _)) => item,
                                    Either::Right((InputSignal::Apply(batch), _)) => {
                                        for part in batch {
                                            apply_input_to_messages(messages, part);
                                        }
                                        continue 'restart;
                                    }
                                    Either::Right((InputSignal::Cancelled, _)) => return,
                                    Either::Right((InputSignal::Closed, _)) => {
                                        input_open = false;
                                        continue;
                                    }
                                }
                            }
                            None => provider_stream.next().await,
                        };

                        match item {
                            Some(item) => {
                                match pump_event(item, &mut structured_buffer).map_err(|err| {
                                    ChatFailure { err, metadata: last_metadata.clone() }
                                })? {
                                    Pump::Done(response) => {
                                        final_response = Some(response);
                                        break;
                                    }
                                    Pump::Yield(event) => yield event,
                                }
                            }
                            None => break,
                        }
                    }

                    if let Some(mut response) = final_response {
                        for v in structured_buffer.drain(..) {
                            response.content.parts.push(PartEnum::Structured(v));
                        }

                        let (results, pass) =
                            self.stream_commit(messages, &response, &mut last_metadata).await?;
                        for fr in results {
                            yield StreamEvent::ToolResult(fr);
                        }
                        if let Some(reason) = pass.pause {
                            yield StreamEvent::Paused(reason);
                            return;
                        }
                        if pass.executed {
                            // Tools ran; another turn so the model reacts.
                            continue 'step;
                        }

                        if let Some(strategy) = self.after_strategy.as_mut() {
                            strategy(messages, last_metadata.as_ref()).await;
                        }
                        yield StreamEvent::Done(response);
                        return;
                    }

                    // No final response and no restart — let the step loop
                    // decide whether to retry.
                    break 'restart;
                }
            }
        }
    }
}

/// Collect the `FunctionResponse`s of resolved tools on the last content — but
/// only when the pass actually executed something, matching the loops' "emit
/// ToolResults for tools that just ran" behavior.
fn collect_tool_results(pass: &ToolCallPass, messages: &Messages) -> Vec<FunctionResponse> {
    if !pass.executed {
        return Vec::new();
    }
    match messages.0.last() {
        Some(last) => last.parts.tools().filter_map(|t| t.response().cloned()).collect(),
        None => Vec::new(),
    }
}

impl<CP: StreamProvider> Chat<CP, Unstructured> {
    /// Output-only streaming chat loop with HITL support.
    ///
    /// Yields each token/chunk as `StreamEvent::TextChunk` / similar. When a
    /// tool strategy pauses execution (for example, `RequireApproval`), the
    /// stream yields `StreamEvent::Paused(PauseReason)` and then terminates.
    /// The caller resolves pending tools on `messages` — typically via
    /// `Messages::find_tool_mut` — and calls `stream()` again to continue.
    ///
    /// This is the engine ([`run_stream`](Self::run_stream)) with no input
    /// channel.
    pub async fn stream<'a>(
        &'a mut self,
        messages: &'a mut Messages,
    ) -> Result<BoxStream<'a, Result<StreamEvent, ChatFailure>>, ChatFailure> {
        if let Some(strategy) = self.before_strategy.as_mut() {
            strategy(messages, None).await;
        }
        Ok(Box::pin(self.run_stream(messages, None)))
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

    /// Minimal `StreamProvider` that yields a pre-loaded event sequence, then
    /// ends. Exercises the engine's accumulator without a network.
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

        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], StreamEvent::Structured(_)));
        assert!(matches!(events[1], StreamEvent::Structured(_)));

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

        assert_eq!(events.len(), 5);
        assert!(matches!(events[0], StreamEvent::TextChunk(ref t) if t == "hello "));
        assert!(matches!(events[1], StreamEvent::Structured(_)));
        assert!(matches!(events[2], StreamEvent::TextChunk(ref t) if t == "world"));
        assert!(matches!(events[3], StreamEvent::Structured(_)));

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
