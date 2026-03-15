use chat_core::{
    error::ChatError,
    types::{
        messages::{
            content::{CompleteReasonEnum, Content},
            embeddings::Embeddings,
        },
        metadata::{Metadata, usage::Usage},
        response::{ChatResponse, EmbeddingsResponse},
    },
};
use serde::Deserialize;

use super::parts::{OpenAIContent, OpenAIResponseMessage};

// ---------------------------------------------------------------------------
// Shared response types (used for both streaming and non-streaming)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct OpenAIResponse {
    pub id: Option<String>,
    pub model: Option<String>,
    pub choices: Vec<OpenAIChoice>,
    pub usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIChoice {
    /// Non-streaming responses use `message`, streaming uses `delta`.
    /// `#[serde(alias)]` lets us deserialize both into the same field.
    #[serde(alias = "delta")]
    pub message: OpenAIResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIUsage {
    pub prompt_tokens: Option<usize>,
    pub completion_tokens: Option<usize>,
    pub total_tokens: Option<usize>,
}

// ---------------------------------------------------------------------------
// Conversion to core types
// ---------------------------------------------------------------------------

impl OpenAIResponse {
    pub fn into_core_chat_response(mut self) -> Result<ChatResponse, ChatError> {
        let choice = self
            .choices
            .pop()
            .ok_or_else(|| ChatError::InvalidResponse("No choices returned by OpenAI".into()))?;

        let mut oai_content = OpenAIContent::from_response_message(choice.message, false)?;

        oai_content.complete_reason = match choice.finish_reason.as_deref() {
            Some("stop") => CompleteReasonEnum::Stop,
            Some("length") => CompleteReasonEnum::MaxTokens,
            Some("tool_calls") => CompleteReasonEnum::ToolCall,
            Some(other) => CompleteReasonEnum::Other(other.to_string()),
            None => CompleteReasonEnum::None,
        };

        let metadata = Metadata {
            id: self.id,
            model_slug: self.model,
            usage: self
                .usage
                .map(|u| Usage {
                    input_tokens: u.prompt_tokens.unwrap_or(0),
                    output_tokens: u.completion_tokens.unwrap_or(0),
                    total_tokens: u.total_tokens.unwrap_or(0),
                })
                .unwrap_or_default(),
            ..Default::default()
        };

        Ok(ChatResponse {
            content: Content::from(oai_content),
            metadata: Some(metadata),
        })
    }
}

// ---------------------------------------------------------------------------
// Embedding response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct OpenAIEmbeddingResponse {
    pub data: Vec<OpenAIEmbeddingData>,
    pub model: Option<String>,
    pub usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIEmbeddingData {
    pub embedding: Vec<f32>,
    pub index: usize,
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
                    input_tokens: u.prompt_tokens.unwrap_or(0),
                    output_tokens: u.completion_tokens.unwrap_or(0),
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
