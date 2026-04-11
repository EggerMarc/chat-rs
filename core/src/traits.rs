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
    },
};
use async_trait::async_trait;
#[cfg(feature = "stream")]
use futures::stream::BoxStream;
use serde_json::Value;

#[async_trait]
pub trait CompletionProvider: Send + Sync {
    /// Run one completion step.
    ///
    /// `tool_declarations`, when `Some`, is the pre-computed
    /// `ToolCollection::json()` output — a JSON array of function
    /// declarations — aggregated across every scoped collection the
    /// chat loop holds. Providers splice this into their native request
    /// format; they never see the typed `ToolCollection<M>` directly.
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&Value>,
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
        tool_declarations: Option<&Value>,
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
    async fn embed(&self, messages: &mut Messages) -> Result<EmbeddingsResponse, ChatFailure>;
}
