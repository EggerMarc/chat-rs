pub struct NoModel;
pub struct WithModel<P>(pub(crate) P);

pub struct Unstructured;
pub struct Structured<T>(pub(crate) std::marker::PhantomData<T>);
pub struct Embedded;

/// Type-state marker for a `Chat` that streams *both* outputs and inputs.
/// Built via [`ChatBuilder::with_input_stream`]. Gates the
/// `Chat<CP, InputStreamed>::stream(&mut messages)` method, which returns a
/// [`ChatStream`](crate::chat::input::ChatStream): the output stream you
/// iterate with `.next()`, carrying an input side you push to with
/// `.send()`. The marker is intentionally not generic — input always rides
/// the channel as `PartEnum`, and mapping any producer (microphone, camera)
/// into `PartEnum` happens caller-side, before `send`.
pub struct InputStreamed;
