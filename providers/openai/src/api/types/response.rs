use chat_core::{
    error::ChatError,
    types::{
        messages::{
            content::{CompleteReasonEnum, Content, RoleEnum},
            embeddings::Embeddings,
            file::{File, UrlData},
            parts::{PartEnum, Parts},
            reasoning::Reasoning,
            text::Text,
        },
        metadata::{Metadata, usage::Usage},
        response::{ChatResponse, EmbeddingsResponse},
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use tools_rs::FunctionCall;

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
    pub message: OpenAIMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIMessage {
    pub role: Option<String>,
    pub content: Option<OpenAIResponseContent>,
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
}

/// OpenAI can return `content` as either a plain string or an array of typed
/// content blocks (for multimodal responses). This enum handles both via
/// `#[serde(untagged)]`.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum OpenAIResponseContent {
    Text(String),
    Parts(Vec<OpenAIContentPart>),
}

/// A single typed content block within an OpenAI response content array.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum OpenAIContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: OpenAIImageUrl },
}

#[derive(Debug, Deserialize)]
pub struct OpenAIImageUrl {
    pub url: String,
}

/// Tool calls — all fields optional to support streaming continuation chunks
/// where only `arguments` fragments arrive without `id` or `name`.
#[derive(Debug, Deserialize)]
pub struct OpenAIToolCall {
    pub index: Option<usize>,
    pub id: Option<String>,
    pub function: Option<OpenAIToolCallFunction>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIToolCallFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
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

        let core_parts = choice.message.into_core_parts(false)?;

        let complete_reason = match choice.finish_reason.as_deref() {
            Some("stop") => CompleteReasonEnum::Stop,
            Some("length") => CompleteReasonEnum::MaxTokens,
            Some("tool_calls") => CompleteReasonEnum::Stop,
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
            content: Content {
                parts: core_parts,
                role: RoleEnum::Model,
                complete_reason,
            },
            metadata: Some(metadata),
        })
    }
}

impl OpenAIMessage {
    /// Convert this message into core `Parts`.
    ///
    /// When `streaming` is true, tool-call arguments are kept as
    /// `Value::String` so `merge_chunk` can concatenate fragments.
    /// When false, arguments are parsed into proper JSON.
    pub fn into_core_parts(self, streaming: bool) -> Result<Parts, ChatError> {
        let mut parts = Parts::default();

        // Reasoning first (mirrors Gemini's thought-before-text ordering)
        if let Some(reasoning) = self.reasoning_content {
            parts.push(PartEnum::Reasoning(Reasoning::new(reasoning)));
        }

        if let Some(content) = self.content {
            match content {
                OpenAIResponseContent::Text(text) => {
                    parts.push(PartEnum::Text(Text::new(&text)));
                }
                OpenAIResponseContent::Parts(content_parts) => {
                    for part in content_parts {
                        parts.push(part.into_core()?);
                    }
                }
            }
        }

        if let Some(tool_calls) = self.tool_calls {
            for tc in tool_calls {
                if let Some(func) = tc.function {
                    let name = func.name.unwrap_or_default();
                    let args_str = func.arguments.unwrap_or_default();

                    let arguments = if streaming {
                        // Keep as string for merge_chunk concatenation
                        Value::String(args_str)
                    } else {
                        serde_json::from_str(&args_str).unwrap_or_else(|_| json!({}))
                    };

                    parts.push(PartEnum::FunctionCall(FunctionCall {
                        id: tc.id.map(Into::into),
                        name,
                        arguments,
                    }));
                }
            }
        }

        Ok(parts)
    }
}

impl OpenAIContentPart {
    pub fn into_core(self) -> Result<PartEnum, ChatError> {
        match self {
            Self::Text { text } => Ok(PartEnum::Text(Text::new(&text))),
            Self::ImageUrl { image_url } => {
                let url_data = UrlData::from_str(&image_url.url).map_err(|e| {
                    ChatError::InvalidResponse(format!("Invalid image URL: {e}"))
                })?;
                Ok(PartEnum::File(File::Url(url_data)))
            }
        }
    }
}

use std::str::FromStr;

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
        let data = self.data.into_iter().next().ok_or_else(|| {
            ChatError::InvalidResponse("No embedding data returned by OpenAI".into())
        })?;

        let dimension = data.embedding.len();

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
                content: data.embedding,
                dimension,
            },
            metadata: Some(metadata),
        })
    }
}
