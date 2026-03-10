use async_trait::async_trait;
use futures::stream::BoxStream;
use tools_rs::ToolCollection;

use crate::{
    error::ChatError,
    types::{
        failure::ChatFailure,
        messages::Messages,
        options::ChatOptions,
        response::{ChatResponse, EmbeddingsResponse, StreamEvent},
    },
};

#[async_trait]
pub trait CompletionProvider: Send + Sync {
    async fn complete(
        &self,
        messages: &mut Messages,
        tools: Option<&ToolCollection>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure>;
}

#[async_trait]
pub trait StreamProvider {
    async fn stream(
        &self,
        messages: &mut Messages,
        tools: Option<&ToolCollection>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError>;
}

#[async_trait]
pub trait EmbeddingsProvider {
    async fn embed(&self, messages: &mut Messages) -> Result<EmbeddingsResponse, ChatFailure>;
}
