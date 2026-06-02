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

/// Thin wrapper over [`ResponsesClient`]. Delegates the Responses API
/// wire (completion + stream) to the inner client and adds the
/// OpenAI-specific `/embeddings` endpoint, which is **not** part of
/// the Responses API.
pub struct OpenAIClient<T: Transport> {
    pub(crate) inner: ResponsesClient<T>,
}

#[async_trait::async_trait]
impl<T: Transport> CompletionProvider for OpenAIClient<T> {
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
impl<T: Transport> chat_core::traits::StreamProvider for OpenAIClient<T> {
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
