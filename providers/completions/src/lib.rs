//! Generic OpenAI-compatible Chat Completions client.
//!
//! Targets the `/v1/chat/completions` wire format implemented by OpenAI,
//! Ollama, vLLM, llama.cpp, LiteLLM, Cerebras, Groq, Together, Fireworks,
//! and the rest of the OAI-compatible ecosystem. Bring your own base URL.
//!
//! ```no_run
//! use chat_completions::ChatCompletionsBuilder;
//!
//! let client = ChatCompletionsBuilder::new()
//!     .with_base_url("http://localhost:8000/v1")
//!     .with_model("my-model")
//!     .with_api_key("sk-...")
//!     .build();
//! ```

mod api;
mod client;

use std::marker::PhantomData;

use chat_core::transport::Transport;
use chat_core::types::provider_meta::ProviderMeta;

pub use chat_core::transport::ReqwestTransport;
pub use crate::client::ChatCompletionsClient;

pub struct WithoutModel;
pub struct WithModel;

pub struct WithoutUrl;
pub struct WithUrl;

pub struct ChatCompletionsBuilder<
    M = WithoutModel,
    U = WithoutUrl,
    T: Transport = ReqwestTransport,
> {
    model_name: Option<String>,
    api_key: Option<String>,
    scheme: String,
    host: String,
    base_path: String,
    extra_headers: Vec<(String, String)>,
    transport: Option<T>,
    meta: ProviderMeta,
    _m: PhantomData<M>,
    _u: PhantomData<U>,
}

impl Default for ChatCompletionsBuilder<WithoutModel, WithoutUrl, ReqwestTransport> {
    fn default() -> Self {
        Self::new()
    }
}

impl ChatCompletionsBuilder<WithoutModel, WithoutUrl, ReqwestTransport> {
    pub fn new() -> Self {
        Self {
            model_name: None,
            api_key: None,
            scheme: String::new(),
            host: String::new(),
            base_path: String::new(),
            extra_headers: Vec::new(),
            transport: Some(ReqwestTransport::default()),
            meta: ProviderMeta::default(),
            _m: PhantomData,
            _u: PhantomData,
        }
    }
}

impl<M, U, T: Transport> ChatCompletionsBuilder<M, U, T> {
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra_headers.push((key.into(), value.into()));
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.meta.description = Some(description.into());
        self
    }

    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl std::any::Any + Send + Sync + 'static,
    ) -> Self {
        self.meta.data.insert(key.into(), Box::new(value));
        self
    }

    /// Supply a custom transport, replacing the default.
    pub fn with_transport<T2: Transport>(self, transport: T2) -> ChatCompletionsBuilder<M, U, T2> {
        ChatCompletionsBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            scheme: self.scheme,
            host: self.host,
            base_path: self.base_path,
            extra_headers: self.extra_headers,
            transport: Some(transport),
            meta: self.meta,
            _m: PhantomData,
            _u: PhantomData,
        }
    }
}

impl<U, T: Transport> ChatCompletionsBuilder<WithoutModel, U, T> {
    pub fn with_model(self, model_name: impl Into<String>) -> ChatCompletionsBuilder<WithModel, U, T> {
        ChatCompletionsBuilder {
            model_name: Some(model_name.into()),
            api_key: self.api_key,
            scheme: self.scheme,
            host: self.host,
            base_path: self.base_path,
            extra_headers: self.extra_headers,
            transport: self.transport,
            meta: self.meta,
            _m: PhantomData,
            _u: PhantomData,
        }
    }
}

impl<M, T: Transport> ChatCompletionsBuilder<M, WithoutUrl, T> {
    /// Set the base URL of the OpenAI-compatible server.
    ///
    /// Example: `http://localhost:11434/v1`, `https://api.cerebras.ai/v1`.
    /// The path portion is preserved as the base, and completion/embedding
    /// endpoints are appended (`/chat/completions`, `/embeddings`).
    pub fn with_base_url(
        self,
        base_url: impl AsRef<str>,
    ) -> ChatCompletionsBuilder<M, WithUrl, T> {
        let parsed = url::Url::parse(base_url.as_ref()).expect("Invalid base URL");
        let scheme = parsed.scheme().to_string();
        let host = parsed
            .host_str()
            .expect("base URL missing host")
            .to_string()
            + &parsed.port().map(|p| format!(":{p}")).unwrap_or_default();
        let base_path = parsed.path().trim_end_matches('/').to_string();

        ChatCompletionsBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            scheme,
            host,
            base_path,
            extra_headers: self.extra_headers,
            transport: self.transport,
            meta: self.meta,
            _m: PhantomData,
            _u: PhantomData,
        }
    }
}

impl<T: Transport> ChatCompletionsBuilder<WithModel, WithUrl, T> {
    /// Build the client.
    ///
    /// Panics if no transport is set and `T` is not the default `ReqwestTransport`.
    pub fn build(self) -> ChatCompletionsClient<T> {
        let transport = self.transport.expect(
            "No transport provided. Call .with_transport() or rely on the default ReqwestTransport.",
        );

        ChatCompletionsClient {
            model_name: self.model_name.unwrap(),
            api_key: self.api_key,
            scheme: self.scheme,
            host: self.host,
            base_path: self.base_path,
            extra_headers: self.extra_headers,
            transport,
            meta: self.meta,
        }
    }
}
