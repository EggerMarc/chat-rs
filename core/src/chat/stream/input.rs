//! `Chat<CP, InputStreamed>::stream` — the input-stream surface.
//!
//! There is no separate loop here: this is the shared engine
//! ([`Chat::run_stream`](super::Chat::run_stream)) with an input channel wired
//! in. This module owns only the input-specific delta — the entry point that
//! creates the channel and wraps the result in a [`ChatStream`], plus the merge
//! helpers (`next_input`, `apply_input_to_messages`) the engine calls when a
//! burst of input arrives.

use futures::{StreamExt, channel::mpsc};

use super::handle::{ChatStream, Input, InputStream, OutputStream};
use crate::{
    chat::{Chat, state::InputStreamed},
    error::ChatFailure,
    traits::StreamProvider,
    types::messages::{
        Messages,
        content::{self, RoleEnum},
        parts::PartEnum,
    },
};

impl<CP: StreamProvider> Chat<CP, InputStreamed> {
    /// Streaming chat loop that also accepts caller-pushed input. Returns a
    /// [`ChatStream`]: iterate it with `.next()` for output events, push with
    /// `.send()`, or `split()` it into independent input/output handles.
    ///
    /// Input vocabulary (case-by-case merge, see `apply_input_to_messages`):
    /// - text / file / structured parts and whole `Content`s → pushed as user
    ///   content, coalescing into the trailing user turn;
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

        // Unbounded by design: `send` is sync, infallible, and never drops —
        // the load-bearing ergonomic. The engine greedily drains the channel on
        // every restart, and this HTTP interrupt-and-restart path is for
        // discrete text/tool input, not high-rate streams (those belong on
        // native-WS, which doesn't use this channel). A bounded channel is a
        // future option if audio-over-HTTP ever becomes real.
        let (tx, rx) = mpsc::unbounded::<Input>();

        let output = self.run_stream(messages, Some(rx));

        Ok(ChatStream {
            input: InputStream { tx },
            output: OutputStream { inner: Box::pin(output) },
        })
    }
}

/// Outcome of draining the input channel for one burst.
pub(super) enum InputSignal {
    /// One or more inputs to merge into `Messages`, then restart the provider.
    Apply(Vec<Input>),
    /// A `cancel()` was received — tear the exchange down.
    Cancelled,
    /// All producers dropped — input is closed for good.
    Closed,
}

/// Park until at least one input is ready, then greedily drain everything
/// already queued, so a burst of inputs triggers a single provider restart
/// instead of one per item (and accretes into one user turn via the coalescing
/// merge). A `Cancel` anywhere short-circuits the whole batch.
///
/// Cancel-safe under `select`: the only await is the blocking `rx.next()`; the
/// greedy drain is synchronous, so dropping this future mid-flight never
/// strands a half-consumed batch.
pub(super) async fn next_input(rx: &mut mpsc::UnboundedReceiver<Input>) -> InputSignal {
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
pub(super) fn apply_input_to_messages(messages: &mut Messages, input: Input) {
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
            response::{ChatResponse, StreamEvent},
            tools::ToolDeclarations,
        },
    };
    use async_trait::async_trait;
    use futures::stream::BoxStream;
    use std::collections::HashMap;
    use std::marker::PhantomData;
    use std::sync::{Arc, Mutex};

    /// One provider stream session. `pend: true` appends an infinite
    /// `stream::pending()` after the events, simulating a long generation that
    /// hasn't finished — which lets queued input win the `select` race
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

    fn chat_with(
        sessions: Vec<Session>,
    ) -> (Chat<MockStreamProvider, InputStreamed>, Arc<Mutex<usize>>) {
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
        drop(stream);

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
