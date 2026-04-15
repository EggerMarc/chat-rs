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
        response::ChatResponse,
    },
};
use serde::Deserialize;
use serde_json::Value;
use tools_rs::{CallId, FunctionCall};

use super::request::STRUCTURED_OUTPUT_TOOL_NAME;

#[derive(Debug, Deserialize)]
pub struct ClaudeResponse {
    pub id: Option<String>,
    pub content: Vec<ClaudeContentBlock>,
    pub model: Option<String>,
    pub stop_reason: Option<String>,
    pub usage: Option<ClaudeUsage>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum ClaudeContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        signature: Option<String>,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}

#[derive(Debug, Deserialize, Clone)]
pub struct ClaudeUsage {
    pub input_tokens: Option<usize>,
    pub output_tokens: Option<usize>,
}

impl ClaudeResponse {
    pub fn into_core_chat_response(self, is_structured: bool) -> Result<ChatResponse, ChatError> {
        let mut core_parts = Parts::default();

        for block in self.content {
            match block {
                ClaudeContentBlock::Text { text } => {
                    core_parts.push(PartEnum::Text(Text::new(&text)));
                }
                ClaudeContentBlock::Thinking {
                    thinking,
                    signature,
                } => {
                    let mut r = Reasoning::new(&thinking);
                    if let Some(sig) = signature {
                        r = r.with_signature(sig);
                    }
                    core_parts.push(PartEnum::Reasoning(r));
                }
                ClaudeContentBlock::ToolUse { id, name, input } => {
                    if is_structured && name == STRUCTURED_OUTPUT_TOOL_NAME {
                        core_parts.push(PartEnum::Structured(input));
                    } else {
                        let call_id = CallId::from(id);
                        core_parts.push(PartEnum::from_function_call(FunctionCall {
                            id: Some(call_id),
                            name,
                            arguments: input,
                        }));
                    }
                }
            }
        }

        let complete_reason = match self.stop_reason.as_deref() {
            Some("end_turn") => CompleteReasonEnum::Stop,
            Some("max_tokens") => CompleteReasonEnum::MaxTokens,
            Some("tool_use") => CompleteReasonEnum::ToolCall,
            Some(other) => CompleteReasonEnum::Other(other.to_string()),
            None => CompleteReasonEnum::None,
        };

        let input_tokens = self
            .usage
            .as_ref()
            .and_then(|u| u.input_tokens)
            .unwrap_or(0);
        let output_tokens = self
            .usage
            .as_ref()
            .and_then(|u| u.output_tokens)
            .unwrap_or(0);

        let metadata = Metadata {
            id: self.id,
            model_slug: self.model,
            usage: Usage {
                input_tokens,
                output_tokens,
                total_tokens: input_tokens + output_tokens,
            },
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
