use crate::api::types::request::GeminiRequest;
use crate::api::types::response::GeminiResponse;
use crate::client::GeminiClient;
use crate::CompletionConfig;
use chat_core::traits::CompletionProvider;
use chat_core::types::failure::ChatFailure;
use chat_core::types::messages::Messages;
use chat_core::types::options::ChatOptions;
use chat_core::types::response::ChatResponse;
use tools_rs::ToolCollection;

#[async_trait::async_trait]
impl CompletionProvider for GeminiClient<_, CompletionConfig> {
    async fn complete(
        &self,
        messages: &mut Messages,
        tools: Option<&ToolCollection>,
        options: Option<ChatOptions>,
        structured_output: Option<schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model_name
        );

        let request_body = GeminiRequest::from_core(
            messages,
            tools,
            &self.native_tools,
            self.function_config,
            options.as_ref(),
            structured_output,
        );

        let res = self
            .http_client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ChatFailure::from_err(e))?;

        let gemini_data: GeminiResponse = res.json().await.map_err(|e| ChatFailure::from_err(e))?;

        Ok(gemini_data.into_core_response())
    }
}
