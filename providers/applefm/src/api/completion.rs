use async_trait::async_trait;
use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::CompletionProvider;
use chat_core::types::messages::Messages;
use chat_core::types::options::ChatOptions;
use chat_core::types::provider_meta::ProviderMeta;
use chat_core::types::response::ChatResponse;
use chat_core::types::tools::ToolDeclarations;

use crate::api::types::{request, response};
use crate::client::AppleFMClient;
use crate::ffi;

#[async_trait]
impl CompletionProvider for AppleFMClient {
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        let request_json = request::from_core(
            &self.config,
            messages,
            options,
            structured_output,
            tool_declarations.is_some(),
        )?;

        // The bridge call blocks (model inference); keep it off the
        // async workers.
        let reply_json = tokio::task::spawn_blocking(move || ffi::complete_json(&request_json))
            .await
            .map_err(|e| {
                ChatFailure::from_err(ChatError::Other(format!("bridge task failed: {e}")))
            })?;

        response::into_core(&self.model_slug(), &reply_json)
    }

    fn metadata(&self) -> Option<&ProviderMeta> {
        Some(&self.meta)
    }
}
