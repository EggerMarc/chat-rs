//! The JSON wire protocol crossing the C boundary.
//!
//! Mirrored by `bridge/Sources/AppleFMBridge/WireTypes.swift` — keep the
//! two in sync.

pub(crate) mod request;
pub(crate) mod response;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize)]
pub(crate) struct WireOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    // Sampling mode, flattened. The bridge picks greedy > top_k > top_p.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub greedy: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

/// Configuration for a long-lived session (created once per
/// conversation, reused across turns).
#[derive(Debug, Serialize)]
pub(crate) struct SessionConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    /// Filesystem path to a `.fmadapter` LoRA package.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lora: Option<String>,
}

/// One turn against an existing session. `message` is the new user
/// message on the incremental path, or a full rendered conversation on
/// first turn / rebuild.
#[derive(Debug, Serialize)]
pub(crate) struct TurnRequest {
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<WireOptions>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SessionCreated {
    pub session: u64,
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

/// Events emitted by the bridge's streaming path, discriminated by `type`.
#[cfg(feature = "stream")]
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum WireStreamEvent {
    /// A text fragment (already diffed from cumulative snapshots).
    Delta {
        text: String,
    },
    /// Stream finished; carries the authoritative full text.
    Done {
        text: String,
        finish: String,
    },
    Error {
        error: ErrorBody,
    },
}

#[cfg(all(test, feature = "stream"))]
mod tests {
    use super::*;

    #[test]
    fn parses_stream_events() {
        let event: WireStreamEvent =
            serde_json::from_str(r#"{"type":"delta","text":"hi"}"#).unwrap();
        assert!(matches!(event, WireStreamEvent::Delta { text } if text == "hi"));

        let event: WireStreamEvent =
            serde_json::from_str(r#"{"type":"done","text":"hi there","finish":"stop"}"#).unwrap();
        assert!(matches!(event, WireStreamEvent::Done { finish, .. } if finish == "stop"));

        let event: WireStreamEvent =
            serde_json::from_str(r#"{"type":"error","error":{"kind":"generation","message":"x"}}"#)
                .unwrap();
        assert!(matches!(event, WireStreamEvent::Error { error } if error.kind == "generation"));
    }
}
