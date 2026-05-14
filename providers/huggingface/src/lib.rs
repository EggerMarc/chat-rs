//! Hugging Face Inference Providers (Router) for chat-rs.
//!
//! Thin wrapper around [`chat_completions`]. HF Router is OpenAI-compatible
//! (`/v1/chat/completions` only — no Responses API, no embeddings on this
//! surface) and dispatches to Together, Novita, Fireworks, SambaNova,
//! Cerebras, Groq, Replicate, and others through a single billing layer.
//!
//! ```no_run
//! use chat_huggingface::HuggingFaceBuilder;
//!
//! // HF_TOKEN env var is read automatically.
//! let client = HuggingFaceBuilder::new()
//!     .with_model("openai/gpt-oss-120b:fastest")
//!     .build();
//! ```
//!
//! ## Model slug
//!
//! HF slugs are `org/model` plus an optional policy suffix:
//!
//! | Suffix         | Behavior                                         |
//! |----------------|--------------------------------------------------|
//! | `:fastest`     | (default) highest throughput provider            |
//! | `:cheapest`    | lowest price-per-output-token provider           |
//! | `:preferred`   | follow account's preferred provider order        |
//! | `:<provider>`  | force a specific provider (e.g. `:sambanova`)    |
//!
//! Examples: `deepseek-ai/DeepSeek-R1:cheapest`,
//! `meta-llama/Llama-3.3-70B-Instruct:novita`.

use std::marker::PhantomData;

use chat_completions::{
    ChatCompletionsBuilder, ChatCompletionsClient, ReqwestTransport, Transport,
};

/// Default HF Inference Router base URL.
pub const DEFAULT_HF_BASE_URL: &str = "https://router.huggingface.co/v1";

const HF_TOKEN_ENV: &str = "HF_TOKEN";

pub struct WithoutModel;
pub struct WithModel;

pub struct HuggingFaceBuilder<M = WithoutModel, T: Transport = ReqwestTransport> {
    base_url: String,
    model: Option<String>,
    api_key: Option<String>,
    extra_headers: Vec<(String, String)>,
    description: Option<String>,
    transport: Option<T>,
    _m: PhantomData<M>,
}

impl Default for HuggingFaceBuilder<WithoutModel, ReqwestTransport> {
    fn default() -> Self {
        Self::new()
    }
}

impl HuggingFaceBuilder<WithoutModel, ReqwestTransport> {
    pub fn new() -> Self {
        Self {
            base_url: DEFAULT_HF_BASE_URL.to_string(),
            model: None,
            api_key: None,
            extra_headers: Vec::new(),
            description: None,
            transport: Some(ReqwestTransport::default()),
            _m: PhantomData,
        }
    }
}

impl<M, T: Transport> HuggingFaceBuilder<M, T> {
    /// Override the API key. If unset, `HF_TOKEN` is read at build time.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Override the base URL. Defaults to `https://router.huggingface.co/v1`.
    /// Useful for testing against a staging environment or a self-hosted
    /// HF-compatible proxy.
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

    pub fn with_transport<T2: Transport>(self, transport: T2) -> HuggingFaceBuilder<M, T2> {
        HuggingFaceBuilder {
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

impl<T: Transport> HuggingFaceBuilder<WithoutModel, T> {
    /// Set the model slug. Format is `org/model[:provider|:fastest|:cheapest|:preferred]`.
    pub fn with_model(self, model: impl Into<String>) -> HuggingFaceBuilder<WithModel, T> {
        HuggingFaceBuilder {
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

impl<T: Transport> HuggingFaceBuilder<WithModel, T> {
    /// Build the client.
    ///
    /// Resolves the API key in this order: explicit `with_api_key()`, then
    /// the `HF_TOKEN` env var. Panics if neither is present — HF Router
    /// requires a token even on the free tier.
    pub fn build(self) -> ChatCompletionsClient<T> {
        let api_key = self
            .api_key
            .or_else(|| std::env::var(HF_TOKEN_ENV).ok())
            .expect("No Hugging Face token. Set HF_TOKEN or call .with_api_key().");

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
