use crate::{error::ChatError, types::metadata::Metadata};

#[derive(Clone, Debug)]
pub struct ChatFailure {
    pub metadata: Option<Metadata>,
    pub err: ChatError,
}
