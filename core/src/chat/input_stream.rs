//! `Chat<CP, InputStreamed<I>>::stream` — engine path that interleaves
//! a caller-supplied input stream of `PartEnum` values with the
//! model's output stream of `StreamEvent`s.
//!
//! Pattern: same as `Chat<CP, Unstructured>::stream` (`super::stream`),
//! but with a `futures::future::select` in the inner loop so input
//! events can interrupt mid-generation. On each input event, the
//! engine merges it into `Messages` per-variant, drops the current
//! provider stream, and re-enters the provider with the updated
//! state. That's the same interrupt-and-restart semantics HITL
//! already uses (`Paused` → caller mutates Messages → `stream()`
//! again), just automated and input-driven.
//!
//! Native-WS providers (OpenAI Realtime, Gemini Live) that keep a
//! single session open across calls can do so in their client state;
//! the engine just sees a series of short `stream()` calls with
//! updated `Messages` between them.

use async_stream::try_stream;
use futures::{Stream, StreamExt, future::Either, stream::BoxStream};

use crate::{
    chat::{Chat, state::InputStreamed},
    error::ChatFailure,
    traits::StreamProvider,
    types::{
        messages::{
            Messages,
            content::{self, RoleEnum},
            parts::PartEnum,
        },
        metadata::Metadata,
        response::{ChatResponse, StreamEvent},
    },
};

impl<CP: StreamProvider, I> Chat<CP, InputStreamed<I>>
where
    I: Stream<Item = PartEnum> + Send + Unpin + 'static,
{
    /// Streaming chat loop that also consumes an input stream of
    /// `PartEnum` values. The input is interleaved with the model's
    /// output: each input event triggers a case-by-case merge into
    /// `Messages` and re-opens the provider stream so the model sees
    /// the updated state.
    ///
    /// Input vocabulary (case-by-case merge):
    /// - `PartEnum::Text` / `File` / `Structured` → pushed as a fresh
    ///   `User` `Content` into `Messages`.
    /// - `PartEnum::Tool` → resolves a matching pending `Tool` part on
    ///   the most recent `Model` content by call-id (set its response).
    /// - `PartEnum::Reasoning` / `Embeddings` → no-op (not meaningful
    ///   as inbound).
    ///
    /// Close: dropping or exhausting the input stream ends input
    /// processing; the engine continues draining the provider until
    /// `Done`. Cancel: drop the returned output stream.
    pub async fn stream<'a>(
        &'a mut self,
        messages: &'a mut Messages,
        mut input: I,
    ) -> Result<BoxStream<'a, Result<StreamEvent, ChatFailure>>, ChatFailure> {
        if let Some(strategy) = self.before_strategy.as_mut() {
            strategy(messages, None).await;
        }

        let mut input_open = true;

        let stream = try_stream! {
            let max_steps = self.max_steps.unwrap_or(1);
            let mut last_metadata: Option<Metadata> = None;

            'step: for _ in 0..max_steps {
                // Pre-step: same as Unstructured::stream — drain any
                // already-resolved tools, yield ToolResult, pause if
                // anything still needs caller action.
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

                // Restart loop: each input event drops the current
                // provider stream and re-enters with mutated Messages.
                'restart: loop {
                    let mut provider_stream = self
                        .model
                        .stream(messages, decls_dyn, self.model_options.as_ref())
                        .await
                        .map_err(|err| ChatFailure { err, metadata: last_metadata.clone() })?;

                    let mut final_response: Option<ChatResponse> = None;

                    loop {
                        if input_open {
                            // Race provider events against input events.
                            let pn = provider_stream.next();
                            let inp = input.next();
                            match futures::future::select(Box::pin(pn), Box::pin(inp)).await {
                                Either::Left((Some(Ok(StreamEvent::Done(resp))), _)) => {
                                    final_response = Some(resp);
                                    break;
                                }
                                Either::Left((Some(Ok(event)), _)) => {
                                    yield event;
                                }
                                Either::Left((Some(Err(err)), _)) => {
                                    Err(ChatFailure { err, metadata: last_metadata.clone() })?;
                                }
                                Either::Left((None, _)) => break,
                                Either::Right((Some(part), _)) => {
                                    apply_input_to_messages(messages, part);
                                    continue 'restart;
                                }
                                Either::Right((None, _)) => {
                                    // Input closed. From here on, poll
                                    // the provider directly — no more
                                    // `select` overhead per event.
                                    input_open = false;
                                }
                            }
                        } else {
                            match provider_stream.next().await {
                                Some(Ok(StreamEvent::Done(resp))) => {
                                    final_response = Some(resp);
                                    break;
                                }
                                Some(Ok(event)) => yield event,
                                Some(Err(err)) => {
                                    Err(ChatFailure { err, metadata: last_metadata.clone() })?;
                                }
                                None => break,
                            }
                        }
                    }

                    // Process the final response — same logic as
                    // Unstructured::stream's post-step.
                    if let Some(response) = final_response {
                        self.model.on_stream_done(&response);

                        if let Some(metadata) = response.metadata.clone() {
                            match &mut last_metadata {
                                Some(existing) => { existing.extend(&metadata); },
                                None => { last_metadata = Some(metadata); },
                            }
                        }

                        messages.push(response.content.clone());

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
                            // Tools just ran; need another turn so the
                            // model sees results. Restart the outer
                            // step loop, not the inner restart loop.
                            continue 'step;
                        }

                        if let Some(strategy) = self.after_strategy.as_mut() {
                            strategy(messages, last_metadata.as_ref()).await;
                        }

                        yield StreamEvent::Done(response);
                        return;
                    }

                    // No final response and no restart — exit the
                    // restart loop and let the outer step loop decide
                    // whether to retry.
                    break 'restart;
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

/// Case-by-case merge: each variant becomes a different mutation on
/// `Messages`. Keep this function small and pattern-matchy — provider
/// authors and downstream consumers will read this to understand the
/// semantics.
fn apply_input_to_messages(messages: &mut Messages, part: PartEnum) {
    match part {
        // User content: push as a fresh User Content. Each input part
        // becomes its own Content so ordering is preserved precisely.
        PartEnum::Text(_) | PartEnum::File(_) | PartEnum::Structured(_) => {
            messages.push(content::from_user([part]));
        }
        // Tool result: resolve a matching pending Tool on the most
        // recent Model content by call-id. If no match, drop silently
        // (provider/caller mismatch — out of scope here).
        PartEnum::Tool(incoming) => {
            let incoming_id = incoming.id.clone();
            let Some(incoming_response) = incoming.response().cloned() else { return };
            for c in messages.0.iter_mut().rev() {
                if c.role != RoleEnum::Model {
                    continue;
                }
                for p in c.parts.0.iter_mut() {
                    if let PartEnum::Tool(existing) = p
                        && existing.id == incoming_id
                        && existing.response().is_none()
                    {
                        existing.complete(incoming_response);
                        return;
                    }
                }
            }
        }
        // Not meaningful as inbound — silently skip.
        PartEnum::Reasoning(_) | PartEnum::Embeddings(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::ChatError,
        types::{
            messages::{
                content::{Content as TestContent, RoleEnum as TestRoleEnum},
                parts::Parts,
            },
            options::ChatOptions,
            tools::ToolDeclarations,
        },
    };
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::marker::PhantomData;
    use std::sync::{Arc, Mutex};

    /// Mock provider that yields a pre-loaded event sequence each time
    /// `stream()` is called. Tracks how many times `stream()` was
    /// invoked so tests can verify the "restart on input" behavior.
    struct MockStreamProvider {
        sessions: Arc<Mutex<Vec<Vec<Result<StreamEvent, ChatError>>>>>,
        invocations: Arc<Mutex<usize>>,
    }

    #[async_trait]
    impl StreamProvider for MockStreamProvider {
        async fn stream(
            &mut self,
            _messages: &mut Messages,
            _tool_declarations: Option<&dyn ToolDeclarations>,
            _options: Option<&ChatOptions>,
        ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError> {
            *self.invocations.lock().unwrap() += 1;
            let events = {
                let mut s = self.sessions.lock().unwrap();
                if s.is_empty() { Vec::new() } else { s.remove(0) }
            };
            Ok(Box::pin(futures::stream::iter(events)))
        }
    }

    fn chat_with(
        sessions: Vec<Vec<Result<StreamEvent, ChatError>>>,
    ) -> (Chat<MockStreamProvider, InputStreamed<futures::stream::Iter<std::vec::IntoIter<PartEnum>>>>, Arc<Mutex<usize>>) {
        let invocations = Arc::new(Mutex::new(0usize));
        let chat = Chat {
            model: MockStreamProvider {
                sessions: Arc::new(Mutex::new(sessions)),
                invocations: invocations.clone(),
            },
            output_shape: None,
            model_options: None,
            max_steps: Some(2),
            max_retries: None,
            retry_strategy: None,
            before_strategy: None,
            after_strategy: None,
            scoped_collections: Vec::new(),
            routing: HashMap::new(),
            _output: PhantomData,
        };
        (chat, invocations)
    }

    fn done(text: &str) -> StreamEvent {
        let mut parts = Parts::default();
        parts.push(PartEnum::from(text.to_string()));
        StreamEvent::Done(ChatResponse {
            content: TestContent {
                role: TestRoleEnum::Model,
                parts,
                complete_reason: Default::default(),
            },
            metadata: None,
        })
    }

    #[tokio::test]
    async fn empty_input_stream_behaves_like_plain_stream() {
        let (mut chat, invocations) = chat_with(vec![vec![
            Ok(StreamEvent::TextChunk("hello".into())),
            Ok(done("hello")),
        ]]);
        let mut messages = Messages::default();
        let input = futures::stream::iter(Vec::<PartEnum>::new());

        let mut s = chat.stream(&mut messages, input).await.expect("stream open");
        let mut events = Vec::new();
        while let Some(ev) = s.next().await {
            events.push(ev.expect("ok"));
        }

        assert_eq!(*invocations.lock().unwrap(), 1, "provider called once");
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], StreamEvent::TextChunk(ref t) if t == "hello"));
        assert!(matches!(events[1], StreamEvent::Done(_)));
    }

    #[test]
    fn apply_text_input_pushes_user_content() {
        let mut messages = Messages::default();
        apply_input_to_messages(&mut messages, PartEnum::from("hello".to_string()));
        assert_eq!(messages.0.len(), 1);
        assert_eq!(messages.0[0].role, RoleEnum::User);
        assert!(matches!(&messages.0[0].parts.0[0], PartEnum::Text(t) if t.0 == "hello"));
    }

    #[test]
    fn apply_structured_input_pushes_user_content() {
        let mut messages = Messages::default();
        let value = serde_json::json!({"action": "move", "to": "kitchen"});
        apply_input_to_messages(&mut messages, PartEnum::Structured(value.clone()));
        assert_eq!(messages.0.len(), 1);
        assert_eq!(messages.0[0].role, RoleEnum::User);
        assert!(matches!(&messages.0[0].parts.0[0], PartEnum::Structured(v) if v == &value));
    }

    #[test]
    fn apply_reasoning_input_is_no_op() {
        let mut messages = Messages::default();
        apply_input_to_messages(
            &mut messages,
            PartEnum::Reasoning(crate::types::messages::reasoning::Reasoning::new(
                "thinking out loud".to_string(),
            )),
        );
        assert!(messages.0.is_empty(), "reasoning should not produce content");
    }

    #[tokio::test]
    async fn input_event_restarts_provider_with_updated_messages() {
        // First session yields a TextChunk then ends (no Done) — the
        // input event will interrupt it. Second session yields Done.
        // We verify the provider was called twice.
        let (mut chat, invocations) = chat_with(vec![
            vec![Ok(StreamEvent::TextChunk("partial".into()))],
            vec![Ok(done("final"))],
        ]);
        let mut messages = Messages::default();
        let input = futures::stream::iter(vec![PartEnum::from("interrupt".to_string())]);

        let mut s = chat.stream(&mut messages, input).await.expect("stream open");
        while let Some(ev) = s.next().await {
            let _ = ev.expect("ok");
        }

        // The provider should have been re-invoked after the input
        // event triggered a restart.
        let n = *invocations.lock().unwrap();
        assert!(n >= 2, "expected at least 2 provider invocations, got {n}");
    }
}
