use crate::types::{
    messages::{content::Content, embeddings::Embeddings},
    metadata::Metadata,
};

#[derive(Clone, Debug)]
pub struct ChatResponse {
    pub metadata: Option<Metadata>,
    pub content: Content,
}

#[derive(Clone, Debug)]
pub struct EmbeddingsResponse {
    pub metadata: Option<Metadata>,
    pub embeddings: Embeddings,
}

#[derive(Debug, Clone)]
pub enum StreamEvent {
    TextChunk(String),
    Done(ChatResponse),
}
