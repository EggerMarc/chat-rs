#[cfg(feature = "stream")]
use crate::error::ChatError;
#[cfg(feature = "stream")]
use crate::types::response::StreamEvent;
use crate::{
    error::ChatFailure,
    types::{
        messages::Messages,
        options::ChatOptions,
        provider_meta::ProviderMeta,
        response::{ChatResponse, EmbeddingsResponse},
        tools::ToolDeclarations,
    },
};
use async_trait::async_trait;
#[cfg(feature = "stream")]
use futures::stream::BoxStream;

/// Tool declarations are the one and only view providers have into the
/// user's tool collections. The trait intentionally exposes nothing
/// about execution, metadata, or strategy — providers translate
/// declarations to their wire format and nothing else. For the
/// builder-side collection API, see [`crate::types::tools`].
#[async_trait]
pub trait CompletionProvider: Send + Sync {
    /// Run one completion step.
    ///
    /// `tool_declarations`, when `Some`, is a view onto every scoped
    /// tool collection the chat loop holds. Call `.json()` on it to get
    /// the JSON array of function declarations that providers splice
    /// into their native request format.
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure>;

    fn metadata(&self) -> Option<&ProviderMeta> {
        None
    }
}

#[cfg(feature = "stream")]
#[async_trait]
pub trait StreamProvider: Send + Sync {
    async fn stream(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError>;

    /// Called after the stream has been fully consumed with the final response.
    /// Providers can override this to store state from the completed stream.
    fn on_stream_done(&mut self, _response: &ChatResponse) {}
}

/// Combined supertrait for providers that support both completion and streaming.
/// All providers that implement both `CompletionProvider` and `StreamProvider`
/// automatically implement this trait via the blanket impl.
#[cfg(feature = "stream")]
pub trait ChatProvider: CompletionProvider + StreamProvider {}

#[cfg(feature = "stream")]
impl<T: CompletionProvider + StreamProvider> ChatProvider for T {}

#[async_trait]
pub trait EmbeddingsProvider: Send + Sync {
    async fn embed(&mut self, messages: &mut Messages) -> Result<EmbeddingsResponse, ChatFailure>;
}
