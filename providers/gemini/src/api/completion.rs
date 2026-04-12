use crate::api::types::error::handle_gemini_error;
use crate::api::types::request::GeminiRequest;
use crate::api::types::response::GeminiCompletionResponse;
use crate::client::GeminiClient;
use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::CompletionProvider;
use chat_core::transport::Transport;
use chat_core::types::provider_meta::ProviderMeta;
use chat_core::types::messages::Messages;
use chat_core::types::options::ChatOptions;
use chat_core::types::response::ChatResponse;
use chat_core::types::tools::ToolDeclarations;

#[async_trait::async_trait]
impl<T: Transport> CompletionProvider for GeminiClient<T> {
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model_name
        );

        let request_body = GeminiRequest::from_core(
            messages,
            tool_declarations,
            Some(self.native_tools.as_slice()),
            self.function_config.as_ref(),
            options,
            structured_output,
            self.include_thoughts,
        )
        .map_err(ChatFailure::from_err)?;

        let body = serde_json::to_vec(&request_body)
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        let req = chat_core::transport::Request {
            url,
            headers: vec![
                ("x-goog-api-key".into(), self.api_key.clone()),
                ("Content-Type".into(), "application/json".into()),
            ],
            body,
        };

        let res = self
            .transport
            .send(req)
            .await
            .map_err(ChatFailure::from_err)?;

        let res = handle_gemini_error(res)?;

        let gemini_data: GeminiCompletionResponse = serde_json::from_slice(&res.body)
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        gemini_data
            .into_core_chat_response()
            .map_err(ChatFailure::from_err)
    }

    fn metadata(&self) -> Option<&ProviderMeta> {
        Some(&self.meta)
    }
}
