//! Ollama provider for chat-rs.
//!
//! Thin wrapper around [`chat_completions`] — Ollama serves an
//! OpenAI-compatible `/v1/chat/completions` endpoint, so all chat,
//! streaming, tools, structured output, and embedding logic lives in the
//! `chat-completions` crate.
//!
//! What this crate adds on top:
//! - Default base URL pointing at the local daemon
//! - `OLLAMA_HOST` env var support
//! - [`OllamaBuilder::pull`] to ensure the model is present before the
//!   first request, hitting Ollama's native `/api/pull`. Returns the
//!   builder so it slots into the normal chain.
//!
//! ```no_run
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! use chat_ollama::OllamaBuilder;
//!
//! let client = OllamaBuilder::new()
//!     .with_model("llama3.2")
//!     .pull().await?
//!     .build();
//! # Ok(()) }
//! ```

use std::marker::PhantomData;

use chat_completions::{
    CompletionsBuilder, CompletionsClient, ChatError, Request, ReqwestTransport, Transport,
    TransportError,
};
use serde::Deserialize;
use serde_json::json;

/// Default Ollama base host when `OLLAMA_HOST` is not set.
pub const DEFAULT_OLLAMA_HOST: &str = "http://localhost:11434";

const OLLAMA_HOST_ENV: &str = "OLLAMA_HOST";

pub struct WithoutModel;
pub struct WithModel;

/// Ollama-flavored builder. Wraps [`CompletionsBuilder`] and adds
/// `/api/pull` integration so a model can be fetched at build time.
pub struct OllamaBuilder<M = WithoutModel, T: Transport = ReqwestTransport> {
    scheme: String,
    host: String,
    model: Option<String>,
    api_key: Option<String>,
    extra_headers: Vec<(String, String)>,
    description: Option<String>,
    transport: Option<T>,
    _m: PhantomData<M>,
}

impl Default for OllamaBuilder<WithoutModel, ReqwestTransport> {
    fn default() -> Self {
        Self::new()
    }
}

impl OllamaBuilder<WithoutModel, ReqwestTransport> {
    /// Build pointed at `OLLAMA_HOST` if set, otherwise `http://localhost:11434`.
    pub fn new() -> Self {
        let host =
            std::env::var(OLLAMA_HOST_ENV).unwrap_or_else(|_| DEFAULT_OLLAMA_HOST.to_string());
        Self::with_host(host)
    }

    /// Build pointed at the given host. Accepts plain `http://host:port`
    /// or a URL with a `/v1` suffix (the suffix is stripped — Ollama's
    /// pull endpoint lives outside `/v1`).
    pub fn with_host(host: impl AsRef<str>) -> Self {
        let parsed = url::Url::parse(host.as_ref()).expect("Invalid Ollama host URL");
        let scheme = parsed.scheme().to_string();
        let host_port = parsed
            .host_str()
            .expect("Ollama host URL missing host")
            .to_string()
            + &parsed.port().map(|p| format!(":{p}")).unwrap_or_default();

        Self {
            scheme,
            host: host_port,
            model: None,
            api_key: None,
            extra_headers: Vec::new(),
            description: None,
            transport: Some(ReqwestTransport::default()),
            _m: PhantomData,
        }
    }
}

impl<M, T: Transport> OllamaBuilder<M, T> {
    /// Confirm the Ollama daemon is reachable at the configured host.
    ///
    /// Hits `/api/version`. Returns `Ok(())` if anything answers
    /// (including 4xx — the daemon is alive, just rejecting the
    /// request), or [`ChatError::Provider`] with an actionable install
    /// hint when the connection is refused.
    pub async fn ping(&self) -> Result<(), ChatError> {
        let transport = self.transport.as_ref().expect("transport set");
        let req = Request {
            scheme: self.scheme.clone(),
            host: self.host.clone(),
            path: "/api/version".to_string(),
            headers: vec![("Content-Type".into(), "application/json".into())],
            body: Vec::new(),
        };
        match transport.send(req).await {
            Ok(_) => Ok(()),
            Err(e) => Err(map_transport_error(&self.scheme, &self.host, e)),
        }
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.push((key.into(), value.into()));
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn with_transport<T2: Transport>(self, transport: T2) -> OllamaBuilder<M, T2> {
        OllamaBuilder {
            scheme: self.scheme,
            host: self.host,
            model: self.model,
            api_key: self.api_key,
            extra_headers: self.extra_headers,
            description: self.description,
            transport: Some(transport),
            _m: PhantomData,
        }
    }
}

impl<T: Transport> OllamaBuilder<WithoutModel, T> {
    pub fn with_model(self, model: impl Into<String>) -> OllamaBuilder<WithModel, T> {
        OllamaBuilder {
            scheme: self.scheme,
            host: self.host,
            model: Some(model.into()),
            api_key: self.api_key,
            extra_headers: self.extra_headers,
            description: self.description,
            transport: self.transport,
            _m: PhantomData,
        }
    }
}

impl<T: Transport> OllamaBuilder<WithModel, T> {
    /// Build the client without contacting the daemon.
    pub fn build(self) -> CompletionsClient<T> {
        let transport = self.transport.expect("transport set");
        let model = self.model.expect("model set");

        let base_url = format!("{}://{}/v1", self.scheme, self.host);
        let mut b = CompletionsBuilder::new()
            .with_base_url(base_url)
            .with_model(model)
            .with_transport(transport);

        if let Some(key) = self.api_key {
            b = b.with_api_key(key);
        }
        for (k, v) in self.extra_headers {
            b = b.with_header(k, v);
        }
        if let Some(desc) = self.description {
            b = b.with_description(desc);
        }
        b.build()
    }

    /// Ensure the configured model is downloaded.
    ///
    /// Issues a `POST /api/pull` against the daemon with `stream: false`.
    /// If the model is already present locally this returns near-instantly;
    /// otherwise it blocks until the download completes (no progress output).
    ///
    /// Returns the builder so the caller can keep chaining — pair with
    /// `.build()` to get a ready-to-use client:
    ///
    /// ```no_run
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// # use chat_ollama::OllamaBuilder;
    /// let client = OllamaBuilder::new()
    ///     .with_model("llama3.2")
    ///     .pull().await?
    ///     .build();
    /// # Ok(()) }
    /// ```
    pub async fn pull(self) -> Result<Self, ChatError> {
        let model = self.model.as_ref().expect("model set");
        let transport = self.transport.as_ref().expect("transport set");

        let body = serde_json::to_vec(&json!({
            "model": model,
            "stream": false,
        }))
        .map_err(|e| ChatError::Other(e.to_string()))?;

        let mut headers = vec![("Content-Type".into(), "application/json".into())];
        if let Some(key) = &self.api_key {
            headers.push(("Authorization".into(), format!("Bearer {key}")));
        }
        headers.extend(self.extra_headers.iter().cloned());

        let req = Request {
            scheme: self.scheme.clone(),
            host: self.host.clone(),
            path: "/api/pull".to_string(),
            headers,
            body,
        };

        let res = transport
            .send(req)
            .await
            .map_err(|e| map_transport_error(&self.scheme, &self.host, e))?;
        if !(200..300).contains(&res.status) {
            let body = String::from_utf8_lossy(&res.body);
            return Err(ChatError::Provider(format!(
                "Ollama pull failed (HTTP {}): {body}",
                res.status
            )));
        }

        #[derive(Deserialize)]
        struct PullResponse {
            #[serde(default)]
            status: Option<String>,
            #[serde(default)]
            error: Option<String>,
        }

        let parsed: PullResponse = serde_json::from_slice(&res.body).unwrap_or(PullResponse {
            status: None,
            error: None,
        });

        if let Some(err) = parsed.error {
            return Err(ChatError::Provider(format!("Ollama pull: {err}")));
        }
        if let Some(status) = parsed.status
            && status != "success"
            && !status.is_empty()
        {
            return Err(ChatError::Provider(format!("Ollama pull status: {status}")));
        }
        Ok(self)
    }
}

/// Translate a transport-level failure into a [`ChatError`] with an
/// install/start hint when the daemon was unreachable.
fn map_transport_error(scheme: &str, host: &str, err: TransportError) -> ChatError {
    match &err {
        TransportError::Connection(msg) => ChatError::Provider(format!(
            "Ollama daemon unreachable at {scheme}://{host} ({msg}). \
             Install from https://ollama.com/download, then run `ollama serve`. \
             Override the host with OLLAMA_HOST=http://your-host:port."
        )),
        _ => ChatError::from(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_host_only() {
        let b = OllamaBuilder::with_host("http://localhost:11434");
        assert_eq!(b.scheme, "http");
        assert_eq!(b.host, "localhost:11434");
    }

    #[test]
    fn parses_host_with_v1_path() {
        let b = OllamaBuilder::with_host("http://localhost:11434/v1");
        assert_eq!(b.scheme, "http");
        assert_eq!(b.host, "localhost:11434");
    }

    #[test]
    fn parses_remote_host() {
        let b = OllamaBuilder::with_host("https://my-ollama.example.com:8443");
        assert_eq!(b.scheme, "https");
        assert_eq!(b.host, "my-ollama.example.com:8443");
    }
}
