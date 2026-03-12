use crate::{
    error::{ChatFailure},
    types::{
        messages::Messages,
        options::ChatOptions,
        response::{ChatResponse, EmbeddingsResponse},
    },
};
use async_trait::async_trait;
#[cfg(feature = "stream")]
use futures::stream::BoxStream;
use tools_rs::ToolCollection;
#[cfg(feature = "stream")]
use crate::types::response::StreamEvent

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

#[cfg(feature="stream")]
#[async_trait]
pub trait StreamProvider: Send + Sync {
    async fn stream(
        &self,
        messages: &mut Messages,
        tools: Option<&ToolCollection>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError>;
}

#[async_trait]
pub trait EmbeddingsProvider: Send + Sync {
    async fn embed(&self, messages: &mut Messages) -> Result<EmbeddingsResponse, ChatFailure>;
}
