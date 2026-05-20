use chat_core::error::{ChatError, ChatFailure};

/// Convert an anyhow error from mistral.rs builder calls into a [`ChatFailure`].
pub fn from_anyhow(context: &str, err: anyhow::Error) -> ChatFailure {
    ChatFailure::from_err(ChatError::Provider(format!("{context}: {err}")))
}

/// Convert a mistral.rs SDK error from a request call into a [`ChatFailure`].
pub fn from_sdk(context: &str, err: impl std::fmt::Display) -> ChatFailure {
    ChatFailure::from_err(ChatError::Provider(format!("{context}: {err}")))
}

/// Invalid / malformed response from the engine — distinct from provider
/// errors because it indicates a parsing bug, not a model failure.
pub fn invalid_response(msg: impl Into<String>) -> ChatFailure {
    ChatFailure::from_err(ChatError::InvalidResponse(msg.into()))
}
