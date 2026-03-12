use crate::{error::ChatError, types::metadata::Metadata};

#[derive(Clone, Debug)]
pub struct ChatFailure {
    pub metadata: Option<Metadata>,
    pub err: ChatError,
}

impl ChatFailure {
    pub fn from_err(err: Box<dyn std::error::Error>) -> Self {
        Self {
            metadata: None,
            err: ChatError::Other(err.to_string()),
        }
    }
}
