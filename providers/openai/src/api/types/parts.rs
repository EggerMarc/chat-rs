use chat_core::types::messages::{
    content::{CompleteReasonEnum, Content, RoleEnum},
    embeddings::Embeddings,
    file::File,
    parts::{PartEnum, Parts},
    reasoning::Reasoning,
    text::Text,
};
use tools_rs::{FunctionCall, FunctionResponse};

// ---------------------------------------------------------------------------
// OpenAI-native part enum — mirrors core's PartEnum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum OpenAIPartEnum {
    Text(String),
    Reasoning(String),
    FunctionCall(FunctionCall),
    FunctionResponse(FunctionResponse),
    File(File),
    Structured(serde_json::Value),
    Embeddings(Embeddings),
}

// ---------------------------------------------------------------------------
// Role
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenAIRole {
    System,
    User,
    Assistant,
    Tool,
}

impl OpenAIRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::System => "system",
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::Tool => "tool",
        }
    }
}

impl From<&RoleEnum> for OpenAIRole {
    fn from(role: &RoleEnum) -> Self {
        match role {
            RoleEnum::System => Self::System,
            RoleEnum::User => Self::User,
            RoleEnum::Model => Self::Assistant,
        }
    }
}

impl From<&OpenAIRole> for RoleEnum {
    fn from(role: &OpenAIRole) -> Self {
        match role {
            OpenAIRole::System => Self::System,
            OpenAIRole::User => Self::User,
            OpenAIRole::Assistant | OpenAIRole::Tool => Self::Model,
        }
    }
}

// ---------------------------------------------------------------------------
// OpenAIContent — one logical message with typed parts
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OpenAIContent {
    pub role: OpenAIRole,
    pub parts: Vec<OpenAIPartEnum>,
    pub complete_reason: CompleteReasonEnum,
}

impl OpenAIContent {
    pub fn new(role: OpenAIRole, parts: Vec<OpenAIPartEnum>) -> Self {
        Self {
            role,
            parts,
            complete_reason: CompleteReasonEnum::None,
        }
    }
}

// ---------------------------------------------------------------------------
// Core -> OpenAI conversion
// ---------------------------------------------------------------------------

impl From<&Content> for OpenAIContent {
    fn from(content: &Content) -> Self {
        let role = OpenAIRole::from(&content.role);
        let parts = content.parts.0.iter().map(OpenAIPartEnum::from).collect();
        Self {
            role,
            parts,
            complete_reason: content.complete_reason.clone(),
        }
    }
}

impl From<&PartEnum> for OpenAIPartEnum {
    fn from(part: &PartEnum) -> Self {
        match part {
            PartEnum::Text(t) => Self::Text(t.0.clone()),
            PartEnum::Reasoning(r) => Self::Reasoning(r.text.clone()),
            PartEnum::FunctionCall(fc) => Self::FunctionCall(fc.clone()),
            PartEnum::FunctionResponse(fr) => Self::FunctionResponse(fr.clone()),
            PartEnum::File(f) => Self::File(f.clone()),
            PartEnum::Structured(v) => Self::Structured(v.clone()),
            PartEnum::Embeddings(e) => Self::Embeddings(e.clone()),
        }
    }
}

impl From<OpenAIContent> for Content {
    fn from(oai: OpenAIContent) -> Self {
        let role = RoleEnum::from(&oai.role);
        let parts = Parts(oai.parts.into_iter().map(PartEnum::from).collect());
        Content {
            role,
            parts,
            complete_reason: oai.complete_reason,
        }
    }
}

impl From<OpenAIPartEnum> for PartEnum {
    fn from(part: OpenAIPartEnum) -> Self {
        match part {
            OpenAIPartEnum::Text(t) => Self::Text(Text::new(t)),
            OpenAIPartEnum::Reasoning(r) => Self::Reasoning(Reasoning::new(r)),
            OpenAIPartEnum::FunctionCall(fc) => Self::FunctionCall(fc),
            OpenAIPartEnum::FunctionResponse(fr) => Self::FunctionResponse(fr),
            OpenAIPartEnum::File(f) => Self::File(f),
            OpenAIPartEnum::Structured(v) => Self::Structured(v),
            OpenAIPartEnum::Embeddings(e) => Self::Embeddings(e),
        }
    }
}
