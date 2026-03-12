use chat_core::{
    error::ChatError,
    types::{
        messages::{
            content::{CompleteReasonEnum, Content, RoleEnum},
            parts::{PartEnum, Parts},
            reasoning::Reasoning,
            text::Text,
        },
        metadata::{Metadata, usage::Usage},
        response::{ChatResponse, EmbeddingsResponse},
    },
};
use serde::Deserialize;
use serde_json::Value;
use tools_rs::FunctionCall;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCompletionResponse {
    pub candidates: Option<Vec<GeminiCandidate>>,
    pub usage_metadata: Option<GeminiUsage>,
    pub model_version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCandidate {
    pub content: Option<GeminiContentResponse>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiContentResponse {
    pub role: Option<String>,
    pub parts: Option<Vec<GeminiPartResponse>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiPartResponse {
    pub text: Option<String>,
    pub function_call: Option<GeminiFunctionCallResponse>,
    pub thought_signature: Option<String>,
    pub thought: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiFunctionCallResponse {
    pub name: String,
    pub args: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiUsage {
    pub prompt_token_count: Option<usize>,
    pub candidates_token_count: Option<usize>,
    pub total_token_count: Option<usize>,
}

impl GeminiCompletionResponse {
    pub fn into_core_chat_response(self) -> Result<ChatResponse, ChatError> {
        let candidate = self
            .candidates
            .and_then(|c| c.into_iter().next())
            .ok_or_else(|| ChatError::InvalidResponse("No candidates returned".into()))?;

        let gemini_content = candidate
            .content
            .ok_or_else(|| ChatError::InvalidResponse("Candidate had no content".into()))?;

        // Extract reusable parts
        let mut core_parts = Parts::default();
        if let Some(parts) = gemini_content.parts {
            for part in parts {
                let thought_signature = part.thought_signature.clone();

                if let Some(text) = part.text {
                    if let Some(thought) = part.thought
                        && thought
                    {
                        core_parts.push(PartEnum::Reasoning(Reasoning {
                            text,
                            signature: thought_signature.clone(),
                        }));
                    } else {
                        core_parts.push(PartEnum::Text(Text::new(&text)));
                    }
                }
                if let Some(fc) = part.function_call {
                    let args = fc.args.unwrap_or_else(|| Value::Object(Default::default()));
                    core_parts.push(PartEnum::from_function_call(FunctionCall {
                        name: fc.name,
                        arguments: args,
                        id: thought_signature.map(Into::into),
                    }));
                }
            }
        }

        let role = match gemini_content.role.as_deref() {
            Some("user") => RoleEnum::User,
            _ => RoleEnum::Model,
        };

        let complete_reason = match candidate.finish_reason.as_deref() {
            Some("STOP") => CompleteReasonEnum::Stop,
            Some("MAX_TOKENS") => CompleteReasonEnum::MaxTokens,
            Some(other) => CompleteReasonEnum::Other(other.to_string()),
            None => CompleteReasonEnum::None,
        };

        let metadata = Metadata {
            model_slug: self.model_version,
            usage: self
                .usage_metadata
                .map(|u| Usage {
                    input_tokens: u.prompt_token_count.unwrap_or(0),
                    output_tokens: u.candidates_token_count.unwrap_or(0),
                    total_tokens: u.total_token_count.unwrap_or(0),
                })
                .unwrap_or_default(),
            ..Default::default()
        };

        Ok(ChatResponse {
            content: Content {
                parts: core_parts,
                role,
                complete_reason,
            },
            metadata: Some(metadata),
        })
    }
}

// ==============================================================================
// 2. EMBEDDINGS RESPONSE (Completely different API endpoint!)
// ==============================================================================

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiEmbeddingResponse {
    pub embedding: GeminiEmbedding,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiEmbedding {
    pub values: Vec<f32>,
}

impl GeminiEmbeddingResponse {
    pub fn into_core_embeddings_response(self) -> Result<EmbeddingsResponse, ChatError> {
        let dimension = self.embedding.values.len();
        Ok(EmbeddingsResponse {
            embeddings: chat_core::types::messages::embeddings::Embeddings {
                content: self.embedding.values,
                dimension,
            },
            metadata: None,
        })
    }
}
