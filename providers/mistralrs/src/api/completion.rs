use async_trait::async_trait;
use chat_core::error::ChatFailure;
use chat_core::traits::CompletionProvider;
use chat_core::types::messages::Messages;
use chat_core::types::options::ChatOptions;
use chat_core::types::provider_meta::ProviderMeta;
use chat_core::types::response::ChatResponse;
use chat_core::types::tools::ToolDeclarations;

use crate::api::types::error::from_sdk;
use crate::api::types::{request, response};
use crate::client::MistralRsClient;

#[async_trait]
impl CompletionProvider for MistralRsClient {
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        let tools_present = tool_declarations.is_some();
        let req = request::from_core(messages, options, structured_output, tools_present)?;

        let raw = self
            .model
            .send_chat_request(req)
            .await
            .map_err(|e| from_sdk("mistral.rs send_chat_request", e))?;

        response::into_core(&self.model_id, raw)
    }

    fn metadata(&self) -> Option<&ProviderMeta> {
        Some(&self.meta)
    }
}
