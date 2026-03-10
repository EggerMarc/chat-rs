use async_trait::async_trait;
use futures::stream::BoxStream;
use tools_rs::ToolCollection;

use crate::{
    error::{ChatError, ChatFailure},
    lib::ChatOptions,
    messages::{Messages, content::Content, embeddings::Embeddings},
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

#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn complete(
        &self,
        messages: &mut Messages,
        tools: Option<&ToolCollection>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure>;
}

#[async_trait]
pub trait ChatStreamProvider {
    async fn stream(
        &self,
        messages: &mut Messages,
        tools: Option<&ToolCollection>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError>;
}
