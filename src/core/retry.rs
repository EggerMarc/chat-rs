use crate::{lib::ChatFailure, messages::Messages};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Clone)]
pub struct RetryContext {
    pub failure: ChatFailure,
    pub idx: u16,
    pub messages: Arc<Messages>,
}

type RetryFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
pub type RetryStrategy = Box<dyn FnMut(RetryContext) -> RetryFuture + Send + Sync>;
