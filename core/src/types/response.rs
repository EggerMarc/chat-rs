use schemars::JsonSchema;
use serde::de::DeserializeOwned;

use crate::types::{
    messages::{content::Content, embeddings::Embeddings},
    metadata::Metadata,
};

#[derive(Clone, Debug)]
pub struct ChatResponse {
    pub metadata: Option<Metadata>,
    pub content: Content,
}

#[derive(Debug, Clone)]
pub struct StructuredResponse<T: DeserializeOwned + JsonSchema> {
    pub content: T,
    pub metadata: Option<Metadata>,
}

#[derive(Clone, Debug)]
pub struct EmbeddingsResponse {
    pub metadata: Option<Metadata>,
    pub embeddings: Embeddings,
}

#[cfg(feature = "stream")]
#[derive(Debug, Clone)]
pub enum StreamEvent {
    TextChunk(String),
    Done(ChatResponse),
}
