use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum ChatError {
    #[error("network error: {0}")]
    Network(String),

    #[error("provider error: {0}")]
    Provider(String),

    #[error("rate limited")]
    RateLimited,

    #[error("invalid response: {0}")]
    InvalidResponse(String),

    #[error("unknown error: {0}")]
    Other(String),
}
