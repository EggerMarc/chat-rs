use crate::api::types::request::GeminiRequest;
use crate::api::types::response::GeminiEmbeddingResponse;
use crate::client::GeminiClient;
use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::{CompletionProvider, EmbeddingsProvider};
use chat_core::types::messages::Messages;
use chat_core::types::options::ChatOptions;
use chat_core::types::response::{ChatResponse, EmbeddingsResponse};
use tools_rs::ToolCollection;

#[async_trait::async_trait]
impl EmbeddingsProvider for GeminiClient {
    async fn embed(&self, messages: &mut Messages) -> Result<EmbeddingsResponse, ChatFailure> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:embedContent",
            self.model_name
        );

        let request_body = GeminiRequest::from_core(messages, None, None, None, None, None)
            .map_err(ChatFailure::from_err)?;

        let res = self
            .http_client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ChatFailure::from_err(ChatError::Network(e.to_string())))?;

        let gemini_data: GeminiEmbeddingResponse = res
            .json()
            .await
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        gemini_data
            .into_core_embeddings_response()
            .map_err(ChatFailure::from_err)
    }
}
