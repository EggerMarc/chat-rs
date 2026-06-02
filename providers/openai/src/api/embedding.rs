use crate::api::types::request::OpenAIEmbeddingRequest;
use crate::api::types::response::OpenAIEmbeddingResponse;
use crate::client::OpenAIClient;
use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::EmbeddingsProvider;
use chat_core::transport::Transport;
use chat_core::types::messages::Messages;
use chat_core::types::response::EmbeddingsResponse;
use chat_responses::handle_responses_error;

#[async_trait::async_trait]
impl<T: Transport> EmbeddingsProvider for OpenAIClient<T> {
    async fn embed(&self, messages: &mut Messages) -> Result<EmbeddingsResponse, ChatFailure> {
        let request_body = OpenAIEmbeddingRequest::from_core(&self.inner.model_name, messages)
            .map_err(ChatFailure::from_err)?;

        let body = serde_json::to_vec(&request_body)
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        let req = chat_core::transport::Request {
            scheme: self.inner.scheme.clone(),
            host: self.inner.host.clone(),
            path: format!("{}/embeddings", self.inner.base_path),
            headers: vec![
                (
                    "Authorization".into(),
                    format!("Bearer {}", self.inner.api_key),
                ),
                ("Content-Type".into(), "application/json".into()),
            ],
            body,
        };

        let res = self
            .inner
            .transport
            .send(req)
            .await
            .map_err(ChatFailure::from_err)?;

        let res = handle_responses_error(res)?;

        let oai_data: OpenAIEmbeddingResponse = serde_json::from_slice(&res.body)
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        oai_data
            .into_core_embeddings_response()
            .map_err(ChatFailure::from_err)
    }
}
