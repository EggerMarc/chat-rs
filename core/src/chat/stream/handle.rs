//! Bidirectional streaming handles for `Chat<CP, InputStreamed>::stream`.
//!
//! `stream()` returns a [`ChatStream`]: it *is* the output stream you
//! iterate with `.next()`, and it carries the input side you push to with
//! `.send()`. [`split`](ChatStream::split) — or [`input`](ChatStream::input)
//! / [`output`](ChatStream::output) — peels the two sides apart so an input
//! producer can run independently of the output consumer (e.g. a microphone
//! task feeding `send()` while the main loop reads events).
//!
//! ## Ownership
//!
//! [`InputStream`] holds only a channel sender — no borrow of `Messages` —
//! so it is `Clone + Send + 'static` and drops cleanly into a spawned task,
//! and clones are independent producers (dropping one never closes the
//! others). [`OutputStream`] owns the engine future and the `&mut Messages`
//! borrow, so it carries lifetime `'a` and is *not* `'static`. The merge
//! into `Messages` happens engine-side, where the borrow lives; the input
//! side only ships `PartEnum`s over the channel. That asymmetry is the whole
//! design — it's why the producer handle is spawnable and the reader is not.

use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{Stream, channel::mpsc, stream::BoxStream};

use crate::{
    error::ChatFailure,
    types::{
        messages::{content::Content, parts::PartEnum},
        response::StreamEvent,
    },
};

/// A single inbound message on the input channel.
///
/// `send` accepts anything convertible into this via [`IntoInput`]. There
/// is deliberately no "stream" variant: a continuous producer is mapped to
/// `PartEnum` *before* it reaches `send` and pumped in by the caller — the
/// handle is `Clone + Send + 'static` precisely so that loop can live in a
/// task. If the library ever owns the pumping, that's a differently-named
/// method; `send` means "send a thing", not "attach a firehose".
#[allow(clippy::large_enum_variant)]
pub enum Input {
    /// A single part. Text/file/structured parts become user content
    /// (coalescing into the trailing user turn); a `Tool` part resolves a
    /// pending tool call by id.
    Item(PartEnum),
    /// A whole `Content`, pushed as-is (coalescing if it shares the
    /// trailing turn's role).
    Content(Content),
    /// Tear down the exchange: the engine drops the provider stream and ends
    /// the output. Sent by [`InputStream::cancel`].
    Cancel,
}

/// Anything you can hand to [`InputStream::send`]. Curated concrete impls
/// only — deliberately no blanket `Stream` impl, so the common single-item
/// sends stay bare (`send("hi")`, `send(part)`, `send(content)`).
pub trait IntoInput {
    fn into_input(self) -> Input;
}

impl IntoInput for Input {
    fn into_input(self) -> Input {
        self
    }
}
impl IntoInput for PartEnum {
    fn into_input(self) -> Input {
        Input::Item(self)
    }
}
impl IntoInput for Content {
    fn into_input(self) -> Input {
        Input::Content(self)
    }
}
impl IntoInput for String {
    fn into_input(self) -> Input {
        Input::Item(PartEnum::from(self))
    }
}
impl IntoInput for &str {
    fn into_input(self) -> Input {
        Input::Item(PartEnum::from(self))
    }
}

/// Returned by [`InputStream::send`] when the output side is gone (dropped
/// or the stream finished). Producers should treat it as a signal to wind
/// down — the idiomatic shape is `if input.send(x).is_err() { break }`.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SendError {
    #[error("input is disconnected: the output stream was dropped or finished")]
    Disconnected,
}

/// The producer side. Holds only a channel sender — `Clone + Send +
/// 'static`, so it moves freely into spawned tasks and can be cloned for
/// multiple concurrent producers.
#[derive(Clone)]
pub struct InputStream {
    pub(crate) tx: mpsc::UnboundedSender<Input>,
}

impl InputStream {
    /// Push input toward the model: a single part, `&str`/`String`, or a
    /// whole `Content`. Returns `Err(SendError::Disconnected)` if the output
    /// side is gone.
    pub fn send(&self, input: impl IntoInput) -> Result<(), SendError> {
        self.tx
            .unbounded_send(input.into_input())
            .map_err(|_| SendError::Disconnected)
    }

    /// Abort the exchange: the provider stream stops and the output ends.
    /// Idempotent — a no-op if already disconnected.
    pub fn cancel(&self) {
        let _ = self.tx.unbounded_send(Input::Cancel);
    }

    /// Whether the output side is still alive.
    pub fn is_connected(&self) -> bool {
        !self.tx.is_closed()
    }
}

/// The read-only side. Owns the engine future (and the `&mut Messages`
/// borrow), so it carries lifetime `'a` and is not `'static`.
pub struct OutputStream<'a> {
    pub(crate) inner: BoxStream<'a, Result<StreamEvent, ChatFailure>>,
}

impl Stream for OutputStream<'_> {
    type Item = Result<StreamEvent, ChatFailure>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().inner.as_mut().poll_next(cx)
    }
}

/// The combined handle returned by `Chat<CP, InputStreamed>::stream`. It
/// *is* the output stream (`.next()`), with the input side carried inside
/// (`.send()` / `.cancel()`). Use [`split`](Self::split) — or
/// [`input`](Self::input) / [`output`](Self::output) — to separate the two
/// sides for independent ownership.
pub struct ChatStream<'a> {
    pub(crate) input: InputStream,
    pub(crate) output: OutputStream<'a>,
}

impl<'a> ChatStream<'a> {
    /// Push input through the wrapper to the inner input side.
    pub fn send(&self, input: impl IntoInput) -> Result<(), SendError> {
        self.input.send(input)
    }

    /// Abort the exchange. See [`InputStream::cancel`].
    pub fn cancel(&self) {
        self.input.cancel();
    }

    /// A fresh producer handle. Non-consuming and cloneable — grab as many
    /// as you need; each is an independent producer to the same exchange.
    /// This is the no-`split` path: keep `self` as the reader and move the
    /// handle into a producer task.
    pub fn input(&self) -> InputStream {
        self.input.clone()
    }

    /// Consume into the read-only half.
    pub fn output(self) -> OutputStream<'a> {
        self.output
    }

    /// Split into independent input and output handles. Sugar over
    /// [`input`](Self::input) + [`output`](Self::output).
    pub fn split(self) -> (InputStream, OutputStream<'a>) {
        (self.input.clone(), self.output)
    }
}

impl Stream for ChatStream<'_> {
    type Item = Result<StreamEvent, ChatFailure>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.get_mut().output).poll_next(cx)
    }
}
