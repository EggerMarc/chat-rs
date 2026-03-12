use crate::api::types::request::GeminiRequest;
use crate::api::types::response::GeminiCompletionResponse;
use crate::client::GeminiClient;
use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::CompletionProvider;
use chat_core::types::messages::Messages;
use chat_core::types::options::ChatOptions;
use chat_core::types::response::ChatResponse;
use tools_rs::ToolCollection;

#[async_trait::async_trait]
impl CompletionProvider for GeminiClient {
    async fn complete(
        &self,
        messages: &mut Messages,
        tools: Option<&ToolCollection>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model_name
        );

        let request_body = GeminiRequest::from_core(
            messages,
            tools,
            Some(self.native_tools.as_slice()),
            self.function_config.as_ref(),
            options,
            structured_output,
        )
        .map_err(ChatFailure::from_err)?;

        let res = self
            .http_client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ChatFailure::from_err(ChatError::Network(e.to_string())))?;

        let gemini_data: GeminiCompletionResponse = res
            .json()
            .await
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        gemini_data
            .into_core_chat_response()
            .map_err(ChatFailure::from_err)
    }
}
