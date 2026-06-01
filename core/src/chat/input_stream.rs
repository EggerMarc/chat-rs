//! `Chat<CP, InputStreamed>::stream` — the bidirectional streaming engine.
//!
//! Same loop as `Chat<CP, Unstructured>::stream` (`super::stream`), but the
//! inner loop races the model's output against an internal input channel via
//! `futures::future::select`, so caller-pushed input can interrupt
//! mid-generation. On an input burst the engine merges it into `Messages`,
//! drops the current provider stream, and re-enters the provider with the
//! updated state — the same interrupt-and-restart semantics HITL uses
//! (`Paused` → caller mutates Messages → `stream()` again), just automated.
//!
//! What survives an interrupt: every completed step — tool calls and their
//! results — is in `Messages` and re-sent on restart. Only the in-flight
//! partial generation is discarded (cheap, side-effect-free; tools execute
//! *between* steps, never during streaming, so an interrupt can never sever
//! a running tool). The model reconciles old and new by re-reading the full
//! transcript; the engine's only job is to never drop history.
//!
//! Input arrives over an `mpsc` channel fed by [`InputStream::send`]. The
//! producer handle is `Clone + Send + 'static` (it borrows nothing), so it
//! drops into a spawned task; the output side owns the `&mut Messages`
//! borrow. Closing all producers flips the engine to draining the provider
//! directly; a `cancel()` tears the whole thing down.

use async_stream::try_stream;
use futures::{StreamExt, channel::mpsc, future::Either};

use crate::{
    chat::{
        Chat,
        input::{ChatStream, Input, InputStream, OutputStream},
        state::InputStreamed,
    },
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

impl<CP: StreamProvider> Chat<CP, InputStreamed> {
    /// Streaming chat loop that also accepts caller-pushed input. Returns a
    /// [`ChatStream`]: iterate it with `.next()` for output events, push with
    /// `.send()`, or `split()` it into independent input/output handles.
    ///
    /// Input vocabulary (case-by-case merge, see `apply_input_to_messages`):
    /// - text / file / structured parts and whole `Content`s → pushed as
    ///   user content, coalescing into the trailing user turn;
    /// - a `Tool` part → resolves a matching pending tool call by id;
    /// - reasoning / embeddings parts → no-op (not meaningful inbound).
    ///
    /// Close: dropping all `InputStream` handles ends input; the engine then
    /// drains the provider directly. Cancel: `cancel()` (or dropping the
    /// output) tears the exchange down.
    pub async fn stream<'a>(
        &'a mut self,
        messages: &'a mut Messages,
    ) -> Result<ChatStream<'a>, ChatFailure> {
        if let Some(strategy) = self.before_strategy.as_mut() {
            strategy(messages, None).await;
        }

        let (tx, mut rx) = mpsc::unbounded::<Input>();

        let stream = try_stream! {
            let max_steps = self.max_steps.unwrap_or(1);
            let mut last_metadata: Option<Metadata> = None;
            let mut input_open = true;

            'step: for _ in 0..max_steps {
                // Pre-step: execute any tools already resolved to Approved on
                // the last Content (typically a just-resolved pause). Emit
                // ToolResult events; pause again if the caller left some
                // tools Pending.
                if let Some(last) = messages.0.last_mut() {
                    let pass = self
                        .tool_call(last)
                        .await
                        .map_err(|err| ChatFailure { err, metadata: last_metadata.clone() })?;

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

                let decls = crate::chat::tool_declarations_from(&self.scoped_collections);
                let decls_dyn = decls
                    .as_ref()
                    .map(|d| d as &dyn crate::types::tools::ToolDeclarations);

                // Restart loop: each input burst drops the current provider
                // stream and re-enters with mutated Messages.
                'restart: loop {
                    let mut provider_stream = self
                        .model
                        .stream(messages, decls_dyn, self.model_options.as_ref())
                        .await
                        .map_err(|err| ChatFailure { err, metadata: last_metadata.clone() })?;

                    let mut final_response: Option<ChatResponse> = None;

                    loop {
                        if input_open {
                            // Race provider events against input. The input
                            // future borrows `rx` (which lives outside the
                            // future), so losing the race and being dropped
                            // is cancel-safe — no queued input is lost.
                            let provider_next = provider_stream.next();
                            let input_next = next_input(&mut rx);
                            match futures::future::select(
                                Box::pin(provider_next),
                                Box::pin(input_next),
                            )
                            .await
                            {
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
                                Either::Right((InputSignal::Apply(batch), _)) => {
                                    for input in batch {
                                        apply_input_to_messages(messages, input);
                                    }
                                    continue 'restart;
                                }
                                Either::Right((InputSignal::Cancelled, _)) => return,
                                Either::Right((InputSignal::Closed, _)) => {
                                    // All producers dropped — stop selecting
                                    // and drain the provider directly.
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

                    if let Some(response) = final_response {
                        self.model.on_stream_done(&response);

                        if let Some(metadata) = response.metadata.clone() {
                            match &mut last_metadata {
                                Some(existing) => {
                                    existing.extend(&metadata);
                                }
                                None => {
                                    last_metadata = Some(metadata);
                                }
                            }
                        }

                        messages.push(response.content.clone());

                        // Post-step: apply strategy to tools the model emitted
                        // this turn.
                        let pass = match messages.0.last_mut() {
                            Some(last) => self
                                .tool_call(last)
                                .await
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
                            // Tools ran; need another turn so the model reacts
                            // to the results.
                            continue 'step;
                        }

                        if let Some(strategy) = self.after_strategy.as_mut() {
                            strategy(messages, last_metadata.as_ref()).await;
                        }

                        yield StreamEvent::Done(response);
                        return;
                    }

                    // No final response and no restart — let the outer step
                    // loop decide whether to retry.
                    break 'restart;
                }
            }
        };

        Ok(ChatStream {
            input: InputStream { tx },
            output: OutputStream { inner: Box::pin(stream) },
        })
    }
}

/// Outcome of draining the input channel for one burst.
enum InputSignal {
    /// One or more inputs to merge into `Messages`, then restart the provider.
    Apply(Vec<Input>),
    /// A `cancel()` was received — tear the exchange down.
    Cancelled,
    /// All producers dropped — input is closed for good.
    Closed,
}

/// Park until at least one input is ready, then greedily drain everything
/// already queued, so a burst of inputs triggers a single provider restart
/// instead of one per item (and accretes into one user turn via the
/// coalescing merge). A `Cancel` anywhere short-circuits the whole batch.
///
/// Cancel-safe under `select`: the only await is the blocking `rx.next()`;
/// the greedy drain is synchronous, so dropping this future mid-flight never
/// strands a half-consumed batch.
async fn next_input(rx: &mut mpsc::UnboundedReceiver<Input>) -> InputSignal {
    let first = match rx.next().await {
        None => return InputSignal::Closed,
        Some(Input::Cancel) => return InputSignal::Cancelled,
        Some(input) => input,
    };
    let mut batch = vec![first];
    while let Ok(extra) = rx.try_recv() {
        if matches!(extra, Input::Cancel) {
            return InputSignal::Cancelled;
        }
        batch.push(extra);
    }
    InputSignal::Apply(batch)
}

/// Case-by-case merge: each input becomes a different mutation on `Messages`.
/// Text/file/structured parts and whole `Content`s go through `Messages::push`,
/// which coalesces same-role content — so a burst accretes into the trailing
/// user turn rather than fragmenting (also keeping strict role-alternation
/// providers happy). A `Tool` part resolves a matching pending call by id.
fn apply_input_to_messages(messages: &mut Messages, input: Input) {
    match input {
        // A whole turn — pushed as-is, coalescing if it shares the trailing
        // role.
        Input::Content(content) => {
            messages.push(content);
        }
        // Tool result: resolve a matching pending Tool on the most recent
        // Model content by call-id. No match → drop silently.
        Input::Item(PartEnum::Tool(incoming)) => {
            let incoming_id = incoming.id.clone();
            let Some(incoming_response) = incoming.response().cloned() else {
                return;
            };
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
        // User content: pushed via `Messages::push` (coalesces into the
        // trailing user turn).
        Input::Item(part @ (PartEnum::Text(_) | PartEnum::File(_) | PartEnum::Structured(_))) => {
            messages.push(content::from_user([part]));
        }
        // Not meaningful as inbound — silently skip.
        Input::Item(PartEnum::Reasoning(_) | PartEnum::Embeddings(_)) => {}
        // Defensive: Cancel is handled in `next_input` before this is called.
        Input::Cancel => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::ChatError,
        types::{
            messages::{content::Content as TestContent, parts::Parts},
            options::ChatOptions,
            tools::ToolDeclarations,
        },
    };
    use async_trait::async_trait;
    use futures::stream::BoxStream;
    use std::collections::HashMap;
    use std::marker::PhantomData;
    use std::sync::{Arc, Mutex};

    /// One provider stream session. `pend: true` appends an infinite
    /// `stream::pending()` after the events, simulating a long generation
    /// that hasn't finished — which lets queued input win the `select` race
    /// deterministically (a plain `iter` is always immediately ready, so the
    /// provider would otherwise always win until exhaustion).
    struct Session {
        events: Vec<Result<StreamEvent, ChatError>>,
        pend: bool,
    }

    impl Session {
        fn ready(events: Vec<Result<StreamEvent, ChatError>>) -> Self {
            Session { events, pend: false }
        }
        fn pending(events: Vec<Result<StreamEvent, ChatError>>) -> Self {
            Session { events, pend: true }
        }
    }

    /// Mock provider that yields one pre-loaded `Session` per `stream()` call
    /// and counts invocations, so tests can assert the restart behavior.
    struct MockStreamProvider {
        sessions: Arc<Mutex<Vec<Session>>>,
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
            let session = {
                let mut s = self.sessions.lock().unwrap();
                if s.is_empty() {
                    Session::ready(Vec::new())
                } else {
                    s.remove(0)
                }
            };
            let base = futures::stream::iter(session.events);
            if session.pend {
                Ok(Box::pin(base.chain(futures::stream::pending())))
            } else {
                Ok(Box::pin(base))
            }
        }
    }

    fn chat_with(sessions: Vec<Session>) -> (Chat<MockStreamProvider, InputStreamed>, Arc<Mutex<usize>>) {
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
                role: RoleEnum::Model,
                parts,
                complete_reason: Default::default(),
            },
            metadata: None,
        })
    }

    #[tokio::test]
    async fn no_input_behaves_like_plain_stream() {
        let (mut chat, invocations) = chat_with(vec![Session::ready(vec![
            Ok(StreamEvent::TextChunk("hello".into())),
            Ok(done("hello")),
        ])]);
        let mut messages = Messages::default();

        // Don't send anything; the combined handle keeps its own input alive,
        // but the provider still drives to Done and the stream ends.
        let mut stream = chat.stream(&mut messages).await.expect("stream open");
        let mut events = Vec::new();
        while let Some(ev) = stream.next().await {
            events.push(ev.expect("ok"));
        }

        assert_eq!(*invocations.lock().unwrap(), 1, "provider called once");
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], StreamEvent::TextChunk(ref t) if t == "hello"));
        assert!(matches!(events[1], StreamEvent::Done(_)));
    }

    #[tokio::test]
    async fn input_restarts_provider_and_merges_into_messages() {
        // Session 1 streams a partial chunk then pends forever; the queued
        // input wins the select, gets merged, and triggers a restart. Session
        // 2 completes.
        let (mut chat, invocations) = chat_with(vec![
            Session::pending(vec![Ok(StreamEvent::TextChunk("partial".into()))]),
            Session::ready(vec![Ok(done("final"))]),
        ]);
        let mut messages = Messages::default();

        let mut stream = chat.stream(&mut messages).await.expect("stream open");
        stream.send("interrupt".to_string()).expect("send");
        while let Some(ev) = stream.next().await {
            let _ = ev.expect("ok");
        }
        drop(stream); // release the &mut messages borrow

        assert_eq!(*invocations.lock().unwrap(), 2, "provider restarted on input");
        assert!(
            messages.0.iter().any(|c| c.role == RoleEnum::User
                && c.parts
                    .0
                    .iter()
                    .any(|p| matches!(p, PartEnum::Text(t) if t.0 == "interrupt"))),
            "the interrupt was merged as user content"
        );
    }

    #[tokio::test]
    async fn cancel_ends_the_stream() {
        // Provider never completes; only cancel can end it.
        let (mut chat, invocations) = chat_with(vec![Session::pending(Vec::new())]);
        let mut messages = Messages::default();

        let mut stream = chat.stream(&mut messages).await.expect("stream open");
        stream.cancel();
        let next = stream.next().await;

        assert!(next.is_none(), "cancel terminates the output");
        assert_eq!(*invocations.lock().unwrap(), 1);
    }

    #[test]
    fn apply_text_input_pushes_user_content() {
        let mut messages = Messages::default();
        apply_input_to_messages(&mut messages, Input::Item(PartEnum::from("hello".to_string())));
        assert_eq!(messages.0.len(), 1);
        assert_eq!(messages.0[0].role, RoleEnum::User);
        assert!(matches!(&messages.0[0].parts.0[0], PartEnum::Text(t) if t.0 == "hello"));
    }

    #[test]
    fn consecutive_text_inputs_coalesce_into_one_turn() {
        let mut messages = Messages::default();
        apply_input_to_messages(&mut messages, Input::Item(PartEnum::from("audio-ish".to_string())));
        apply_input_to_messages(
            &mut messages,
            Input::Item(PartEnum::from("actually, that".to_string())),
        );
        // Both land in a single user turn, distinct parts preserved.
        assert_eq!(messages.0.len(), 1);
        assert_eq!(messages.0[0].role, RoleEnum::User);
        assert_eq!(messages.0[0].parts.0.len(), 2);
    }

    #[test]
    fn apply_content_input_pushes_turn() {
        let mut messages = Messages::default();
        apply_input_to_messages(
            &mut messages,
            Input::Content(content::from_user(["hi", "there"])),
        );
        assert_eq!(messages.0.len(), 1);
        assert_eq!(messages.0[0].role, RoleEnum::User);
        assert_eq!(messages.0[0].parts.0.len(), 2);
    }

    #[test]
    fn apply_reasoning_input_is_no_op() {
        let mut messages = Messages::default();
        apply_input_to_messages(
            &mut messages,
            Input::Item(PartEnum::Reasoning(
                crate::types::messages::reasoning::Reasoning::new("thinking".to_string()),
            )),
        );
        assert!(messages.0.is_empty());
    }
}
