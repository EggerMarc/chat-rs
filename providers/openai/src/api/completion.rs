use crate::api::types::request::OpenAIRequest;
use crate::api::types::response::OpenAIResponse;
use crate::client::OpenAIClient;
use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::CompletionProvider;
use chat_core::types::messages::Messages;
use chat_core::types::options::ChatOptions;
use chat_core::types::response::ChatResponse;
use tools_rs::ToolCollection;

#[async_trait::async_trait]
impl CompletionProvider for OpenAIClient {
    async fn complete(
        &self,
        messages: &mut Messages,
        tools: Option<&ToolCollection>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        let url = format!("{}/chat/completions", self.base_url);

        let request_body = OpenAIRequest::from_core(
            &self.model_name,
            messages,
            tools,
            self.native_tools.as_slice(),
            self.reasoning_effort.clone(),
            options,
            structured_output,
        )
        .map_err(ChatFailure::from_err)?;

        let res = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {:?}", self.api_key))
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ChatFailure::from_err(ChatError::Network(e.to_string())))?;

        let res = res
            .error_for_status()
            .map_err(|e| ChatFailure::from_err(ChatError::Provider(e.to_string())))?;

        let oai_data: OpenAIResponse = res
            .json()
            .await
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        oai_data
            .into_core_chat_response()
            .map_err(ChatFailure::from_err)
    }
}
