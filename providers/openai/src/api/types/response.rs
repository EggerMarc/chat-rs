use chat_core::{
    error::ChatError,
    types::{
        messages::embeddings::Embeddings,
        metadata::{Metadata, usage::Usage},
        response::EmbeddingsResponse,
    },
};
use chat_responses::ResponsesUsage;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct OpenAIEmbeddingResponse {
    pub data: Vec<OpenAIEmbeddingData>,
    pub model: Option<String>,
    pub usage: Option<ResponsesUsage>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIEmbeddingData {
    pub embedding: Vec<f32>,
}

impl OpenAIEmbeddingResponse {
    pub fn into_core_embeddings_response(self) -> Result<EmbeddingsResponse, ChatError> {
        let mut data = self.data.into_iter();
        let first = data.next().ok_or_else(|| {
            ChatError::InvalidResponse("No embedding data returned by OpenAI".into())
        })?;
        if data.next().is_some() {
            return Err(ChatError::InvalidResponse(
                "Expected a single embedding result".into(),
            ));
        }

        let dimension = first.embedding.len();

        let metadata = Metadata {
            model_slug: self.model,
            usage: self
                .usage
                .map(|u| Usage {
                    input_tokens: u.input_tokens.unwrap_or(0),
                    output_tokens: u.output_tokens.unwrap_or(0),
                    total_tokens: u.total_tokens.unwrap_or(0),
                })
                .unwrap_or_default(),
            ..Default::default()
        };

        Ok(EmbeddingsResponse {
            embeddings: Embeddings {
                content: first.embedding,
                dimension,
            },
            metadata: Some(metadata),
        })
    }
}
