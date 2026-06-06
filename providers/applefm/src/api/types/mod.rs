//! The JSON wire protocol crossing the C boundary.
//!
//! Mirrored by `bridge/Sources/AppleFMBridge/WireTypes.swift` — keep the
//! two in sync.

pub(crate) mod request;
pub(crate) mod response;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub(crate) struct WireOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub greedy: Option<bool>,
}

#[derive(Debug, Serialize)]
pub(crate) struct WireMessage {
    /// "user" | "assistant"
    pub role: &'static str,
    pub text: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct CompleteRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// Filesystem path to a `.fmadapter` LoRA package.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lora: Option<String>,
    pub messages: Vec<WireMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<WireOptions>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CompleteReply {
    pub text: String,
    pub finish: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ErrorBody {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ErrorReply {
    pub error: ErrorBody,
}
