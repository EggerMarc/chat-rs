use base64::{Engine as _, engine::general_purpose::STANDARD};
use chat_core::{
    error::ChatError,
    types::{
        messages::{
            content::{CompleteReasonEnum, Content, RoleEnum},
            file::File,
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
    pub inline_data: Option<GeminiInlineDataResponse>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiInlineDataResponse {
    pub mime_type: Option<String>,
    pub data: String,
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
            .and_then(|mut c| c.pop())
            .ok_or_else(|| ChatError::InvalidResponse("No candidates returned".into()))?;

        let mut core_parts = Parts::default();
        let mut role = RoleEnum::Model;

        if let Some(gemini_content) = candidate.content {
            role = match gemini_content.role.as_deref() {
                Some("user") => RoleEnum::User,
                _ => RoleEnum::Model,
            };

            if let Some(parts) = gemini_content.parts {
                for part in parts {
                    let thought_signature = part.thought_signature.clone();

                    if let Some(text) = part.text {
                        // 2. Stable Rust compatible boolean check
                        if part.thought.unwrap_or(false) {
                            core_parts.push(PartEnum::Reasoning(Reasoning {
                                text,
                                signature: thought_signature.clone(),
                            }));
                        } else {
                            core_parts.push(PartEnum::Text(Text::new(&text)));
                        }
                    }

                    if let Some(inline) = part.inline_data {
                        match STANDARD.decode(&inline.data) {
                            Ok(bytes) => {
                                let mime = inline
                                    .mime_type
                                    .unwrap_or_else(|| "application/octet-stream".to_string());
                                core_parts
                                    .push(PartEnum::File(File::from_bytes_with_mime(bytes, mime)));
                            }
                            Err(err) => {
                                tracing::warn!(?err, "failed to decode Gemini inlineData");
                            }
                        }
                    }

                    if let Some(fc) = part.function_call {
                        let args = fc.args.unwrap_or_else(|| Value::Object(Default::default()));
                        core_parts.push(PartEnum::from_function_call(FunctionCall {
                            name: fc.name,
                            arguments: args,
                            id: thought_signature.clone().map(Into::into),
                        }));
                    }
                }
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use chat_core::types::messages::file::FileSource;

    #[test]
    fn inline_image_part_decoded_to_file_image() {
        let body = r#"{
            "candidates": [
                {
                    "content": {
                        "role": "model",
                        "parts": [
                            { "inlineData": { "mimeType": "image/png", "data": "aGk=" } }
                        ]
                    },
                    "finishReason": "STOP"
                }
            ]
        }"#;

        let resp: GeminiCompletionResponse = serde_json::from_str(body).unwrap();
        let core = resp.into_core_chat_response().unwrap();

        let file = core
            .content
            .parts
            .into_iter()
            .find_map(|p| match p {
                PartEnum::File(f) => Some(f),
                _ => None,
            })
            .expect("expected a File part");

        assert!(file.is_image());
        assert_eq!(file.mime.as_str(), "image/png");
        match file.source {
            FileSource::Bytes(bytes) => assert_eq!(bytes, b"hi"),
            other => panic!("expected Bytes source, got {other:?}"),
        }
    }
}
