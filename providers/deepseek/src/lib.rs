//! DeepSeek for chat-rs.
//!
//! Thin wrapper around [`chat_completions`]. DeepSeek serves an
//! OpenAI-compatible `/v1/chat/completions` endpoint; this crate just
//! presets the base URL and the `DEEPSEEK_API_KEY` env var.
//!
//! The DeepSeek V4 series of open-weight models natively emits
//! DSML-tagged (XML-ish) tool calls — `<｜DSML｜tool_calls>` /
//! `<｜DSML｜invoke>` / `<｜DSML｜parameter>`. The hosted API parses
//! that into standard OpenAI-shape `tool_calls` JSON before returning,
//! so this client does not see DSML on the wire. Local runtimes that
//! load the raw weights (e.g. via `chat-mistralrs`) need their own
//! DSML parser — that lives in the local-runtime layer, not here.
//!
//! ```no_run
//! use chat_deepseek::DeepSeekBuilder;
//!
//! // DEEPSEEK_API_KEY env var is read automatically.
//! let client = DeepSeekBuilder::new()
//!     .with_model("deepseek-v4-pro")
//!     .build();
//! ```

use std::marker::PhantomData;

use chat_completions::{
    CompletionsBuilder, CompletionsClient, ReqwestTransport, Transport,
};

/// Default DeepSeek base URL.
pub const DEFAULT_DEEPSEEK_BASE_URL: &str = "https://api.deepseek.com/v1";

const DEEPSEEK_API_KEY_ENV: &str = "DEEPSEEK_API_KEY";

pub struct WithoutModel;
pub struct WithModel;

pub struct DeepSeekBuilder<M = WithoutModel, T: Transport = ReqwestTransport> {
    base_url: String,
    model: Option<String>,
    api_key: Option<String>,
    extra_headers: Vec<(String, String)>,
    description: Option<String>,
    transport: Option<T>,
    _m: PhantomData<M>,
}

impl Default for DeepSeekBuilder<WithoutModel, ReqwestTransport> {
    fn default() -> Self {
        Self::new()
    }
}

impl DeepSeekBuilder<WithoutModel, ReqwestTransport> {
    pub fn new() -> Self {
        Self {
            base_url: DEFAULT_DEEPSEEK_BASE_URL.to_string(),
            model: None,
            api_key: None,
            extra_headers: Vec::new(),
            description: None,
            transport: Some(ReqwestTransport::default()),
            _m: PhantomData,
        }
    }
}

impl<M, T: Transport> DeepSeekBuilder<M, T> {
    /// Override the API key. If unset, `DEEPSEEK_API_KEY` is read at build time.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Override the base URL. Defaults to `https://api.deepseek.com/v1`.
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

    pub fn with_transport<T2: Transport>(self, transport: T2) -> DeepSeekBuilder<M, T2> {
        DeepSeekBuilder {
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

impl<T: Transport> DeepSeekBuilder<WithoutModel, T> {
    pub fn with_model(self, model: impl Into<String>) -> DeepSeekBuilder<WithModel, T> {
        DeepSeekBuilder {
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

impl<T: Transport> DeepSeekBuilder<WithModel, T> {
    /// Build the client.
    ///
    /// Resolves the API key in this order: explicit `with_api_key()`, then
    /// the `DEEPSEEK_API_KEY` env var. Panics if neither is present.
    pub fn build(self) -> CompletionsClient<T> {
        let api_key = self
            .api_key
            .or_else(|| std::env::var(DEEPSEEK_API_KEY_ENV).ok())
            .expect("No DeepSeek API key. Set DEEPSEEK_API_KEY or call .with_api_key().");

        let transport = self.transport.expect("transport set");
        let model = self.model.expect("model set");

        let mut b = CompletionsBuilder::new()
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
