use crate::types::metadata::Metadata;
use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum ChatError {
    #[error("network error: {0}")]
    Network(String),

    #[error("provider error: {0}")]
    Provider(String),

    #[error("rate limited")]
    RateLimited,

    #[error("exceeded maximum steps")]
    MaxStepsExceeded,

    #[error("invalid response: {0}")]
    InvalidResponse(String),

    #[error("error in callback function: {0}")]
    Callback(String),

    #[error("unknown error: {0}")]
    Other(String),
}

#[derive(Clone, Debug, Error)]
#[error("Chat engine failed: {err}")]
pub struct ChatFailure {
    pub metadata: Option<Metadata>,
    #[source]
    pub err: ChatError,
}

impl ChatFailure {
    pub fn from_err<E: Into<ChatError>>(err: E) -> Self {
        Self {
            metadata: None,
            err: err.into(),
        }
    }
}
