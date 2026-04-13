pub struct NoModel;
pub struct WithModel<P>(pub(crate) P);

pub struct Unstructured;
pub struct Structured<T>(pub(crate) std::marker::PhantomData<T>);
pub struct Streamed;
pub struct Embedded;
