use crate::{lib::ChatFailure, messages::Messages, metadata::Metadata};
use std::future::Future;
use std::pin::Pin;

#[derive(Clone)]
pub struct CallbackStepContext {
    pub step: u16,
}

#[derive(Clone)]
pub struct CallbackRetryContext {
    pub idx: u16,
    pub failure: ChatFailure,
}

type CallbackFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
pub type CallbackStrategy =
    Box<dyn Fn(&mut Messages, Option<&Metadata>) -> CallbackFuture + Send + Sync>;
pub type RetryStrategy = Box<
    dyn Fn(&mut Messages, Option<&Metadata>, CallbackRetryContext) -> CallbackFuture + Send + Sync,
>;
