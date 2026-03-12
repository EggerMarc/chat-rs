pub struct NoModel;
pub struct WithModel<P>(pub(crate) P);

pub struct Unstructured;
pub struct Structured<T>(pub(crate) std::marker::PhantomData<T>);
//#[cfg(feature = "stream")] -- TODO add feature flags
pub struct Streamed;
//#[cfg(feature = "embed")]
pub struct Embedded;
