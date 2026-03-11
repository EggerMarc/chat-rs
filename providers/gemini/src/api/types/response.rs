use chat_core::{
    error::ChatError,
    types::{
        messages::{
            content::{CompleteReasonEnum, Content, RoleEnum},
            parts::{PartEnum, Parts},
            text::Text,
        },
        metadata::{usage::Usage, Metadata},
        response::ChatResponse,
    },
};
use serde::Deserialize;
use serde_json::Value;
use tools_rs::FunctionCall;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiResponse {
    pub candidates: Option<Vec<GeminiCandidate>>,
    pub usage_metadata: Option<GeminiUsage>,
    pub model_version: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiCandidate {
    pub content: Option<GeminiContent>,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiContent {
    pub role: Option<String>,
    pub parts: Option<Vec<GeminiPart>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiPart {
    pub text: Option<String>,
    pub function_call: Option<GeminiFunctionCall>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeminiFunctionCall {
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

impl GeminiResponse {
    pub fn into_core_response(self) -> Result<ChatResponse, ChatError> {
        let candidate = self
            .candidates
            .and_then(|mut c| c.pop())
            .ok_or_else(|| ChatError::InvalidResponse("No candidates returned".into()))?;

        let gemini_content = candidate
            .content
            .ok_or_else(|| ChatError::InvalidResponse("Candidate had no content".into()))?;

        let mut core_parts = Parts::default();
        if let Some(parts) = gemini_content.parts {
            for part in parts {
                if let Some(text) = part.text {
                    core_parts.push(PartEnum::Text(Text::new(&text)));
                }
                if let Some(fc) = part.function_call {
                    let args = fc.args.unwrap_or_else(|| Value::Object(Default::default()));
                    core_parts.push(PartEnum::from_function_call(FunctionCall::new(
                        fc.name, args,
                    )));
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

        let content = Content {
            parts: core_parts,
            role,
            complete_reason,
        };

        let metadata = Metadata {
            model_slug: self.model_version,
            usage: self.usage_metadata.map(|u| Usage {
                input_tokens: u.prompt_token_count.unwrap_or(0),
                output_tokens: u.candidates_token_count.unwrap_or(0),
                total_tokens: u.total_token_count.unwrap_or(0),
            }),
            ..Default::default()
        };

        Ok(ChatResponse {
            content,
            metadata: Some(metadata),
        })
    }
}
