use chat_core::{
    error::ChatError,
    types::{
        messages::{
            content::{CompleteReasonEnum, Content, RoleEnum},
            embeddings::Embeddings,
            parts::{PartEnum, Parts},
            text::Text,
        },
        metadata::{Metadata, usage::Usage},
        response::{ChatResponse, EmbeddingsResponse},
    },
};
use serde::Deserialize;
use serde_json::{Value, json};
use tools_rs::FunctionCall;

#[derive(Debug, Deserialize)]
pub struct OpenAIResponse {
    pub id: Option<String>,
    pub model: Option<String>,
    pub choices: Vec<OpenAIChoice>,
    pub usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIChoice {
    pub message: OpenAIAssistantMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIAssistantMessage {
    pub role: Option<String>,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIToolCall {
    pub id: String,
    pub function: OpenAIToolCallFunction,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIUsage {
    pub prompt_tokens: Option<usize>,
    pub completion_tokens: Option<usize>,
    pub total_tokens: Option<usize>,
}

impl OpenAIResponse {
    pub fn into_core_chat_response(mut self) -> Result<ChatResponse, ChatError> {
        let choice = self
            .choices
            .pop()
            .ok_or_else(|| ChatError::InvalidResponse("No choices returned by OpenAI".into()))?;

        let mut core_parts = Parts::default();

        if let Some(text) = choice.message.content {
            core_parts.push(PartEnum::Text(Text::new(&text)));
        }

        if let Some(tool_calls) = choice.message.tool_calls {
            for tc in tool_calls {
                let args_json: Value =
                    serde_json::from_str(&tc.function.arguments).unwrap_or_else(|_| json!({}));

                core_parts.push(PartEnum::FunctionCall(FunctionCall {
                    id: Some(tc.id.into()),
                    name: tc.function.name,
                    arguments: args_json,
                }));
            }
        }

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

// --- Streaming chunk types ---

#[derive(Debug, Deserialize)]
pub struct OpenAIStreamChunk {
    pub id: Option<String>,
    pub model: Option<String>,
    pub choices: Vec<OpenAIStreamChoice>,
    pub usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIStreamChoice {
    pub delta: OpenAIStreamDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIStreamDelta {
    pub role: Option<String>,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<OpenAIStreamToolCall>>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIStreamToolCall {
    pub index: Option<usize>,
    pub id: Option<String>,
    pub function: Option<OpenAIStreamToolCallFunction>,
}

#[derive(Debug, Deserialize)]
pub struct OpenAIStreamToolCallFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

impl OpenAIStreamChunk {
    pub fn into_core_chat_response(mut self) -> Result<ChatResponse, ChatError> {
        let choice = self.choices.pop().ok_or_else(|| {
            ChatError::InvalidResponse("No choices in stream chunk".into())
        })?;

        let mut core_parts = Parts::default();

        if let Some(text) = choice.delta.content {
            core_parts.push(PartEnum::Text(Text::new(&text)));
        }

        if let Some(tool_calls) = choice.delta.tool_calls {
            for tc in tool_calls {
                if let Some(func) = tc.function {
                    let name = func.name.unwrap_or_default();
                    let args_str = func.arguments.unwrap_or_default();

                    // During streaming, arguments arrive incrementally.
                    // We store the raw string fragment as text in arguments;
                    // the merge_chunk logic in Parts will accumulate them.
                    let args_json: Value =
                        serde_json::from_str(&args_str).unwrap_or_else(|_| Value::String(args_str));

                    core_parts.push(PartEnum::FunctionCall(FunctionCall {
                        id: tc.id.map(Into::into),
                        name,
                        arguments: args_json,
                    }));
                }
            }
        }

        let complete_reason = match choice.finish_reason.as_deref() {
            Some("stop") => CompleteReasonEnum::Stop,
            Some("length") => CompleteReasonEnum::MaxTokens,
            Some("tool_calls") => CompleteReasonEnum::Stop,
            Some(other) => CompleteReasonEnum::Other(other.to_string()),
            None => CompleteReasonEnum::None,
        };

        let metadata = if self.usage.is_some() || self.id.is_some() || self.model.is_some() {
            Some(Metadata {
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
            })
        } else {
            None
        };

        Ok(ChatResponse {
            content: Content {
                parts: core_parts,
                role: RoleEnum::Model,
                complete_reason,
            },
            metadata,
        })
    }
}

// --- Embedding response types ---

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
