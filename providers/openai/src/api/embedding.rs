use crate::api::types::error::handle_openai_error;
use crate::api::types::request::OpenAIEmbeddingRequest;
use crate::api::types::response::OpenAIEmbeddingResponse;
use crate::client::OpenAIClient;
use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::EmbeddingsProvider;
use chat_core::types::messages::Messages;
use chat_core::types::response::EmbeddingsResponse;

#[async_trait::async_trait]
impl EmbeddingsProvider for OpenAIClient {
    async fn embed(&self, messages: &mut Messages) -> Result<EmbeddingsResponse, ChatFailure> {
        let url = format!("{}/embeddings", self.base_url);

        let request_body =
            OpenAIEmbeddingRequest::from_core(&self.model_name, messages)
                .map_err(ChatFailure::from_err)?;

        let res = self
            .http_client
            .post(&url)
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .json(&request_body)
            .send()
            .await
            .map_err(|e| ChatFailure::from_err(ChatError::Network(e.to_string())))?;

        let res = handle_openai_error(res).await?;

        let oai_data: OpenAIEmbeddingResponse = res
            .json()
            .await
            .map_err(|e| ChatFailure::from_err(ChatError::InvalidResponse(e.to_string())))?;

        oai_data
            .into_core_embeddings_response()
            .map_err(ChatFailure::from_err)
    }
}
