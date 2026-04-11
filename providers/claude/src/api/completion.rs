use crate::api::types::error::handle_claude_error;
use crate::api::types::request::ClaudeRequest;
use crate::api::types::response::ClaudeResponse;
use crate::client::ClaudeClient;
use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::CompletionProvider;
use chat_core::types::provider_meta::ProviderMeta;
use chat_core::types::messages::Messages;
use chat_core::types::options::ChatOptions;
use chat_core::types::response::ChatResponse;
use serde_json::Value;

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";
const THINKING_BETA_HEADER: &str = "interleaved-thinking-2025-05-14";

#[async_trait::async_trait]
impl CompletionProvider for ClaudeClient {
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&Value>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        let request_body = ClaudeRequest::from_core(
            &self.model_name,
            messages,
            tool_declarations,
            options,
            structured_output,
            false,
            self.thinking_budget,
        )
        .map_err(ChatFailure::from_err)?;

        let mut req = self
            .http_client
            .post(CLAUDE_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", &self.api_version)
            .header("content-type", "application/json");

        if self.include_thoughts {
            req = req.header("anthropic-beta", THINKING_BETA_HEADER);
        }

        let res = req
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ChatFailure::from_err(ChatError::Network(e.to_string())))?;

        let res = handle_claude_error(res).await?;

        let claude_data: ClaudeResponse = res
            .json()
            .await
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        claude_data
            .into_core_chat_response(structured_output.is_some())
            .map_err(ChatFailure::from_err)
    }

    fn metadata(&self) -> Option<&ProviderMeta> {
        Some(&self.meta)
    }
}
