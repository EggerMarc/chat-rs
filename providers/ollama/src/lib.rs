//! Ollama provider for chat-rs.
//!
//! Thin wrapper around [`chat_completions`] — Ollama serves an
//! OpenAI-compatible `/v1/chat/completions` endpoint. This crate only
//! adds Ollama-specific defaults: the local socket URL and the
//! `OLLAMA_HOST` env var convention.
//!
//! ```no_run
//! use chat_ollama::OllamaBuilder;
//!
//! let client = OllamaBuilder::new()
//!     .with_model("llama3")
//!     .build();
//! ```
//!
//! Point at a remote daemon by setting `OLLAMA_HOST=http://host:port`
//! before constructing the builder, or call [`OllamaBuilder::with_host`]
//! explicitly.

pub use chat_completions::{
    ChatCompletionsBuilder, ChatCompletionsClient, ReqwestTransport, WithModel, WithUrl,
    WithoutModel,
};

/// Default Ollama base URL when `OLLAMA_HOST` is not set.
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434/v1";

const OLLAMA_HOST_ENV: &str = "OLLAMA_HOST";

/// Constructor for an Ollama-flavored [`ChatCompletionsBuilder`].
///
/// `OllamaBuilder` is a namespace, not a stateful struct — its
/// constructors return the underlying builder already in `WithUrl` state,
/// so the rest of the chain is the standard chat-completions API
/// (`.with_model`, `.with_api_key`, `.with_transport`, `.build`).
pub struct OllamaBuilder;

impl OllamaBuilder {
    /// Build for the local Ollama daemon (or whatever `OLLAMA_HOST` points at).
    ///
    /// Reads `OLLAMA_HOST` if set, otherwise uses `http://localhost:11434/v1`.
    /// The trailing `/v1` segment is appended if the host string omits it.
    pub fn new() -> ChatCompletionsBuilder<WithoutModel, WithUrl, ReqwestTransport> {
        let host = std::env::var(OLLAMA_HOST_ENV)
            .unwrap_or_else(|_| DEFAULT_OLLAMA_URL.to_string());
        Self::with_host(host)
    }

    /// Build pointed at the given host. Accepts plain `http://host:port`
    /// (the `/v1` suffix is added) or a full URL ending in `/v1`.
    pub fn with_host(
        host: impl AsRef<str>,
    ) -> ChatCompletionsBuilder<WithoutModel, WithUrl, ReqwestTransport> {
        ChatCompletionsBuilder::new().with_base_url(normalize_url(host.as_ref()))
    }
}

fn normalize_url(host: &str) -> String {
    let trimmed = host.trim_end_matches('/');
    if trimmed.ends_with("/v1") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/v1")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_adds_v1_when_missing() {
        assert_eq!(normalize_url("http://localhost:11434"), "http://localhost:11434/v1");
        assert_eq!(normalize_url("http://localhost:11434/"), "http://localhost:11434/v1");
    }

    #[test]
    fn normalize_preserves_v1_suffix() {
        assert_eq!(normalize_url("http://localhost:11434/v1"), "http://localhost:11434/v1");
        assert_eq!(normalize_url("http://localhost:11434/v1/"), "http://localhost:11434/v1");
    }
}
