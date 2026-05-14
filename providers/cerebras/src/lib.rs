//! Cerebras Inference for chat-rs.
//!
//! Thin wrapper around [`chat_completions`]. Cerebras serves an
//! OpenAI-compatible `/v1/chat/completions` endpoint with extremely
//! fast token throughput on their wafer-scale silicon. No embeddings
//! exposed on this surface; no Responses API.
//!
//! ```no_run
//! use chat_cerebras::CerebrasBuilder;
//!
//! // CEREBRAS_API_KEY env var is read automatically.
//! let client = CerebrasBuilder::new()
//!     .with_model("llama-3.3-70b")
//!     .build();
//! ```

use std::marker::PhantomData;

use chat_completions::{
    ChatCompletionsBuilder, ChatCompletionsClient, ReqwestTransport, Transport,
};

/// Default Cerebras Inference base URL.
pub const DEFAULT_CEREBRAS_BASE_URL: &str = "https://api.cerebras.ai/v1";

const CEREBRAS_API_KEY_ENV: &str = "CEREBRAS_API_KEY";

pub struct WithoutModel;
pub struct WithModel;

pub struct CerebrasBuilder<M = WithoutModel, T: Transport = ReqwestTransport> {
    base_url: String,
    model: Option<String>,
    api_key: Option<String>,
    extra_headers: Vec<(String, String)>,
    description: Option<String>,
    transport: Option<T>,
    _m: PhantomData<M>,
}

impl Default for CerebrasBuilder<WithoutModel, ReqwestTransport> {
    fn default() -> Self {
        Self::new()
    }
}

impl CerebrasBuilder<WithoutModel, ReqwestTransport> {
    pub fn new() -> Self {
        Self {
            base_url: DEFAULT_CEREBRAS_BASE_URL.to_string(),
            model: None,
            api_key: None,
            extra_headers: Vec::new(),
            description: None,
            transport: Some(ReqwestTransport::default()),
            _m: PhantomData,
        }
    }
}

impl<M, T: Transport> CerebrasBuilder<M, T> {
    /// Override the API key. If unset, `CEREBRAS_API_KEY` is read at build time.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Override the base URL. Defaults to `https://api.cerebras.ai/v1`.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
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

    pub fn with_transport<T2: Transport>(self, transport: T2) -> CerebrasBuilder<M, T2> {
        CerebrasBuilder {
            base_url: self.base_url,
            model: self.model,
            api_key: self.api_key,
            extra_headers: self.extra_headers,
            description: self.description,
            transport: Some(transport),
            _m: PhantomData,
        }
    }
}

impl<T: Transport> CerebrasBuilder<WithoutModel, T> {
    pub fn with_model(self, model: impl Into<String>) -> CerebrasBuilder<WithModel, T> {
        CerebrasBuilder {
            base_url: self.base_url,
            model: Some(model.into()),
            api_key: self.api_key,
            extra_headers: self.extra_headers,
            description: self.description,
            transport: self.transport,
            _m: PhantomData,
        }
    }
}

impl<T: Transport> CerebrasBuilder<WithModel, T> {
    /// Build the client.
    ///
    /// Resolves the API key in this order: explicit `with_api_key()`, then
    /// the `CEREBRAS_API_KEY` env var. Panics if neither is present.
    pub fn build(self) -> ChatCompletionsClient<T> {
        let api_key = self
            .api_key
            .or_else(|| std::env::var(CEREBRAS_API_KEY_ENV).ok())
            .expect("No Cerebras API key. Set CEREBRAS_API_KEY or call .with_api_key().");

        let transport = self.transport.expect("transport set");
        let model = self.model.expect("model set");

        let mut b = ChatCompletionsBuilder::new()
            .with_base_url(self.base_url)
            .with_model(model)
            .with_api_key(api_key)
            .with_transport(transport);

        for (k, v) in self.extra_headers {
            b = b.with_header(k, v);
        }
        if let Some(desc) = self.description {
            b = b.with_description(desc);
        }
        b.build()
    }
}
