use crate::api::types::error::handle_gemini_error;
use crate::api::types::request::GeminiEmbeddingRequest;
use crate::api::types::response::GeminiEmbeddingResponse;
use crate::client::GeminiClient;
use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::EmbeddingsProvider;
use chat_core::transport::Transport;
use chat_core::types::messages::Messages;
use chat_core::types::response::EmbeddingsResponse;

#[async_trait::async_trait]
impl<T: Transport> EmbeddingsProvider for GeminiClient<T> {
    async fn embed(&self, messages: &mut Messages) -> Result<EmbeddingsResponse, ChatFailure> {
        let path = format!("{}/models/{}:embedContent", self.base_path, self.model_name);

        let request_body =
            GeminiEmbeddingRequest::from_core(messages, self.embeddings_config.as_ref())
                .map_err(ChatFailure::from_err)?;

        let body = serde_json::to_vec(&request_body)
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        let req = chat_core::transport::Request {
            scheme: self.scheme.clone(),
            host: self.host.clone(),
            path,
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

        let gemini_data: GeminiEmbeddingResponse = serde_json::from_slice(&res.body)
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        gemini_data
            .into_core_embeddings_response()
            .map_err(ChatFailure::from_err)
    }
}
