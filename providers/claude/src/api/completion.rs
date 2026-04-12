use crate::api::types::error::handle_claude_error;
use crate::api::types::request::ClaudeRequest;
use crate::api::types::response::ClaudeResponse;
use crate::client::ClaudeClient;
use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::CompletionProvider;
use chat_core::transport::Transport;
use chat_core::types::provider_meta::ProviderMeta;
use chat_core::types::messages::Messages;
use chat_core::types::options::ChatOptions;
use chat_core::types::response::ChatResponse;
use chat_core::types::tools::ToolDeclarations;

const THINKING_BETA_HEADER: &str = "interleaved-thinking-2025-05-14";

#[async_trait::async_trait]
impl<T: Transport> CompletionProvider for ClaudeClient<T> {
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
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

        let body = serde_json::to_vec(&request_body)
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        let mut headers = vec![
            ("x-api-key".into(), self.api_key.clone()),
            ("anthropic-version".into(), self.api_version.clone()),
            ("content-type".into(), "application/json".into()),
        ];

        if self.include_thoughts {
            headers.push(("anthropic-beta".into(), THINKING_BETA_HEADER.into()));
        }

        let req = chat_core::transport::Request {
            host: self.host.clone(),
            path: format!("{}/messages", self.base_path),
            headers,
            body,
        };

        let res = self
            .transport
            .send(req)
            .await
            .map_err(ChatFailure::from_err)?;

        let res = handle_claude_error(res)?;

        let claude_data: ClaudeResponse = serde_json::from_slice(&res.body)
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        claude_data
            .into_core_chat_response(structured_output.is_some())
            .map_err(ChatFailure::from_err)
    }

    fn metadata(&self) -> Option<&ProviderMeta> {
        Some(&self.meta)
    }
}
