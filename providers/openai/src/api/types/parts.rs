use base64::{Engine as _, engine::general_purpose::STANDARD};
use chat_core::{
    error::ChatError,
    types::messages::{
        content::{CompleteReasonEnum, Content, RoleEnum},
        embeddings::Embeddings,
        file::{File, UrlData},
        parts::{PartEnum, Parts},
        reasoning::Reasoning,
        text::Text,
    },
};
use serde::Serialize;
use serde_json::{Value, json};
use std::str::FromStr;
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
    Structured(Value),
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
        let parts = content
            .parts
            .0
            .iter()
            .map(OpenAIPartEnum::from)
            .collect();
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

// ---------------------------------------------------------------------------
// OpenAI -> Core conversion
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Serialization: OpenAIContent -> OpenAI wire format messages
// ---------------------------------------------------------------------------

/// A single OpenAI wire-format message, ready for JSON serialization.
#[derive(Debug, Serialize)]
pub struct OpenAIWireMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl OpenAIContent {
    /// Serialize to one or more OpenAI wire messages.
    ///
    /// FunctionResponse parts become separate `role:"tool"` messages because
    /// OpenAI requires each tool result in its own message.
    pub fn to_wire_messages(&self) -> Vec<OpenAIWireMessage> {
        let mut messages = Vec::new();

        let mut text_parts: Vec<String> = Vec::new();
        let mut vision_parts: Vec<Value> = Vec::new();
        let mut tool_calls: Vec<Value> = Vec::new();
        let mut tool_responses: Vec<&FunctionResponse> = Vec::new();

        for part in &self.parts {
            match part {
                OpenAIPartEnum::Text(t) => text_parts.push(t.clone()),
                OpenAIPartEnum::Reasoning(r) => text_parts.push(r.clone()),
                OpenAIPartEnum::FunctionCall(fc) => {
                    tool_calls.push(json!({
                        "id": fc.id.clone().map(String::from),
                        "type": "function",
                        "function": {
                            "name": fc.name,
                            "arguments": serde_json::to_string(&fc.arguments).unwrap_or_default()
                        }
                    }));
                }
                OpenAIPartEnum::FunctionResponse(fr) => {
                    tool_responses.push(fr);
                }
                OpenAIPartEnum::File(File::Url(u)) => {
                    vision_parts.push(json!({
                        "type": "image_url",
                        "image_url": { "url": u.url.to_string() }
                    }));
                }
                OpenAIPartEnum::File(File::Bytes(b)) => {
                    let b64 = STANDARD.encode(&b.bytes);
                    let uri = format!("data:{};base64,{}", b.mimetype, b64);
                    vision_parts.push(json!({
                        "type": "image_url",
                        "image_url": { "url": uri }
                    }));
                }
                OpenAIPartEnum::Structured(_) | OpenAIPartEnum::Embeddings(_) => {}
            }
        }

        // Build the primary message if there's any content
        if !text_parts.is_empty() || !vision_parts.is_empty() || !tool_calls.is_empty() {
            let content = if !vision_parts.is_empty() {
                // Prepend text items as content blocks before vision parts
                let mut parts = Vec::new();
                for text in &text_parts {
                    parts.push(json!({ "type": "text", "text": text }));
                }
                parts.extend(vision_parts);
                Some(Value::Array(parts))
            } else if !text_parts.is_empty() {
                Some(Value::String(text_parts.join("\n")))
            } else {
                None
            };

            let wire_tool_calls = if tool_calls.is_empty() {
                None
            } else {
                Some(tool_calls)
            };

            messages.push(OpenAIWireMessage {
                role: self.role.as_str().to_string(),
                content,
                name: None,
                tool_calls: wire_tool_calls,
                tool_call_id: None,
            });
        }

        // Each FunctionResponse becomes its own tool message
        for fr in tool_responses {
            let content_str = if fr.result.is_string() {
                fr.result.as_str().unwrap().to_string()
            } else {
                fr.result.to_string()
            };

            messages.push(OpenAIWireMessage {
                role: "tool".to_string(),
                content: Some(Value::String(content_str)),
                tool_call_id: Some(
                    fr.id
                        .clone()
                        .map(Into::into)
                        .unwrap_or_else(|| "call_unknown".to_string()),
                ),
                name: Some(fr.name.clone()),
                tool_calls: None,
            });
        }

        messages
    }
}

// ---------------------------------------------------------------------------
// Deserialization: OpenAI JSON response -> OpenAIContent
// ---------------------------------------------------------------------------

impl OpenAIContent {
    /// Parse an OpenAI response message (flat struct with optional fields)
    /// into a typed `OpenAIContent`.
    ///
    /// When `streaming` is true, tool-call arguments are kept as
    /// `Value::String` so stream merging can concatenate fragments.
    pub fn from_response_message(
        msg: OpenAIResponseMessage,
        streaming: bool,
    ) -> Result<Self, ChatError> {
        let role = match msg.role.as_deref() {
            Some("assistant") => OpenAIRole::Assistant,
            Some("system") => OpenAIRole::System,
            Some("user") => OpenAIRole::User,
            Some("tool") => OpenAIRole::Tool,
            _ => OpenAIRole::Assistant,
        };

        let mut parts = Vec::new();

        // Reasoning first (mirrors Gemini's thought-before-text ordering)
        if let Some(reasoning) = msg.reasoning_content {
            parts.push(OpenAIPartEnum::Reasoning(reasoning));
        }

        if let Some(content) = msg.content {
            match content {
                OpenAIResponseContent::Text(text) => {
                    parts.push(OpenAIPartEnum::Text(text));
                }
                OpenAIResponseContent::Parts(content_parts) => {
                    for part in content_parts {
                        match part {
                            OpenAIContentPart::Text { text } => {
                                parts.push(OpenAIPartEnum::Text(text));
                            }
                            OpenAIContentPart::ImageUrl { image_url } => {
                                let url_data = UrlData::from_str(&image_url.url).map_err(|e| {
                                    ChatError::InvalidResponse(format!("Invalid image URL: {e}"))
                                })?;
                                parts.push(OpenAIPartEnum::File(File::Url(url_data)));
                            }
                        }
                    }
                }
            }
        }

        if let Some(tool_calls) = msg.tool_calls {
            for tc in tool_calls {
                if let Some(func) = tc.function {
                    let name = func.name.ok_or_else(|| {
                        ChatError::InvalidResponse("Missing tool call function name".into())
                    })?;
                    let args_str = func.arguments.ok_or_else(|| {
                        ChatError::InvalidResponse("Missing tool call arguments".into())
                    })?;

                    let arguments = if streaming {
                        Value::String(args_str)
                    } else {
                        serde_json::from_str(&args_str).map_err(|e| {
                            ChatError::InvalidResponse(format!(
                                "Invalid tool-call arguments JSON: {e}"
                            ))
                        })?
                    };

                    parts.push(OpenAIPartEnum::FunctionCall(FunctionCall {
                        id: tc.id.map(Into::into),
                        name,
                        arguments,
                    }));
                }
            }
        }

        Ok(Self {
            role,
            parts,
            complete_reason: CompleteReasonEnum::None,
        })
    }
}

// ---------------------------------------------------------------------------
// Raw deserialization types (OpenAI wire format)
// ---------------------------------------------------------------------------

use serde::Deserialize;

/// Raw OpenAI response message — flat struct for deserialization only.
#[derive(Debug, Deserialize)]
pub struct OpenAIResponseMessage {
    pub role: Option<String>,
    pub content: Option<OpenAIResponseContent>,
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
}

/// OpenAI can return `content` as either a plain string or an array of typed
/// content blocks (for multimodal responses).
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

/// Tool calls — all fields optional to support streaming continuation chunks.
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
