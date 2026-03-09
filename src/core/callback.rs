use crate::{lib::ChatFailure, messages::Messages};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

#[derive(Clone)]
pub struct CallbackContext {
    pub messages: Arc<Messages>,
}

#[derive(Clone)]
pub struct CallbackStepContext {
    pub messages: Arc<Messages>,
    pub step: u16,
}

#[derive(Clone)]
pub struct CallbackRetryContext {
    pub messages: Arc<Messages>,
    pub idx: u16,
    pub failure: ChatFailure,
}

type CallbackFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
pub type CallbackStrategy = Box<dyn FnMut(CallbackContext) -> CallbackFuture + Send + Sync>;
pub type RetryStrategy = Box<dyn FnMut(CallbackRetryContext) -> CallbackFuture + Send + Sync>;
pub type StepStrategy = Box<dyn FnMut(CallbackStepContext) -> CallbackFuture + Send + Sync>;

/*
with_before_step() -> Can hold
with_after_step()

with_before()
with_after()

with_retry()
*/
