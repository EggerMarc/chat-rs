pub struct NoModel;
pub struct WithModel<P>(pub(crate) P);

pub struct Unstructured;
pub struct Structured<T>(pub(crate) std::marker::PhantomData<T>);
pub struct Streamed;
pub struct Embedded;

/// Type-state marker for a `Chat` that streams *both* outputs and inputs.
/// Built via [`ChatBuilder::with_input_stream`]. Gates the
/// `Chat<CP, InputStreamed<I>>::stream(&mut messages, input)` method,
/// which interleaves the model's output stream with a caller-supplied
/// `Stream<Item = PartEnum>` input source.
pub struct InputStreamed<I>(pub(crate) std::marker::PhantomData<fn() -> I>);
