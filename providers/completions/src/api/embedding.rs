use crate::api::types::error::handle_error;
use crate::api::types::request::CompletionsEmbeddingRequest;
use crate::api::types::response::CompletionsEmbeddingResponse;
use crate::client::CompletionsClient;
use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::EmbeddingsProvider;
use chat_core::transport::Transport;
use chat_core::types::messages::Messages;
use chat_core::types::response::EmbeddingsResponse;

#[async_trait::async_trait]
impl<T: Transport> EmbeddingsProvider for CompletionsClient<T> {
    async fn embed(&self, messages: &mut Messages) -> Result<EmbeddingsResponse, ChatFailure> {
        let request_body = CompletionsEmbeddingRequest::from_core(&self.model_name, messages)
            .map_err(ChatFailure::from_err)?;

        let body = serde_json::to_vec(&request_body)
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        let req = chat_core::transport::Request {
            scheme: self.scheme.clone(),
            host: self.host.clone(),
            path: format!("{}/embeddings", self.base_path),
            headers: self.build_headers(),
            body,
        };

        let res = self
            .transport
            .send(req)
            .await
            .map_err(ChatFailure::from_err)?;

        let res = handle_error(res)?;

        let parsed: CompletionsEmbeddingResponse = serde_json::from_slice(&res.body)
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        parsed
            .into_core_embeddings_response()
            .map_err(ChatFailure::from_err)
    }
}
