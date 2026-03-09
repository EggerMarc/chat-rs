use crate::{lib::ChatFailure, messages::Messages};
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
pub type CallbackStrategy = Box<dyn Fn(&mut Messages) -> CallbackFuture + Send + Sync>;
pub type RetryStrategy =
    Box<dyn Fn(&mut Messages, CallbackRetryContext) -> CallbackFuture + Send + Sync>;
pub type StepStrategy =
    Box<dyn Fn(&mut Messages, CallbackStepContext) -> CallbackFuture + Send + Sync>;

/*
with_before_step() -> Can hold
with_after_step()

with_before()
with_after()

with_retry()
*/
