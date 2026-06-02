use chat_core::{
    error::ChatFailure,
    traits::CompletionProvider,
    transport::Transport,
    types::{
        messages::Messages, options::ChatOptions, provider_meta::ProviderMeta,
        response::ChatResponse, tools::ToolDeclarations,
    },
};
use chat_responses::ResponsesClient;

/// Thin wrapper over [`ResponsesClient`]. OpenRouter exposes an
/// OpenAI-compatible Responses API (Beta) at
/// `https://openrouter.ai/api/v1/responses`, so the wire (completion +
/// stream) is delegated wholesale to the inner client. The wrapper
/// exists to give OpenRouter its own named client type and a home for
/// any future OpenRouter-specific surface.
pub struct OpenRouterClient<T: Transport> {
    pub(crate) inner: ResponsesClient<T>,
}

#[async_trait::async_trait]
impl<T: Transport> CompletionProvider for OpenRouterClient<T> {
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        self.inner
            .complete(messages, tool_declarations, options, structured_output)
            .await
    }

    fn metadata(&self) -> Option<&ProviderMeta> {
        self.inner.metadata()
    }
}

#[cfg(feature = "stream")]
#[async_trait::async_trait]
impl<T: Transport> chat_core::traits::StreamProvider for OpenRouterClient<T> {
    async fn stream(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
    ) -> Result<
        futures::stream::BoxStream<
            'static,
            Result<chat_core::types::response::StreamEvent, chat_core::error::ChatError>,
        >,
        chat_core::error::ChatError,
    > {
        self.inner
            .stream(messages, tool_declarations, options)
            .await
    }

    fn on_stream_done(&mut self, response: &ChatResponse) {
        self.inner.on_stream_done(response)
    }
}
