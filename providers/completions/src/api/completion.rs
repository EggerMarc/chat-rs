use crate::api::types::error::handle_error;
use crate::api::types::request::{ChatCompletionsRequest, CompletionsRequestConfig};
use crate::api::types::response::ChatCompletionsResponse;
use crate::client::ChatCompletionsClient;
use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::CompletionProvider;
use chat_core::transport::Transport;
use chat_core::types::messages::Messages;
use chat_core::types::options::ChatOptions;
use chat_core::types::provider_meta::ProviderMeta;
use chat_core::types::response::ChatResponse;
use chat_core::types::tools::ToolDeclarations;

#[async_trait::async_trait]
impl<T: Transport> CompletionProvider for ChatCompletionsClient<T> {
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        let request_body = ChatCompletionsRequest::from_core(CompletionsRequestConfig {
            model_name: &self.model_name,
            messages,
            tool_declarations,
            options,
            output_shape: structured_output,
        })
        .map_err(ChatFailure::from_err)?;

        let body = serde_json::to_vec(&request_body)
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        let req = chat_core::transport::Request {
            scheme: self.scheme.clone(),
            host: self.host.clone(),
            path: format!("{}/chat/completions", self.base_path),
            headers: self.build_headers(),
            body,
        };

        let res = self
            .transport
            .send(req)
            .await
            .map_err(ChatFailure::from_err)?;

        let res = handle_error(res)?;

        let parsed: ChatCompletionsResponse = serde_json::from_slice(&res.body)
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        parsed
            .into_core_chat_response()
            .map_err(ChatFailure::from_err)
    }

    fn metadata(&self) -> Option<&ProviderMeta> {
        Some(&self.meta)
    }
}
