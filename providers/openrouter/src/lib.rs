//! OpenRouter provider for chat-rs.
//!
//! Thin wrapper over [`chat_responses`] targeting OpenRouter's
//! OpenAI-compatible **Responses API (Beta)** at
//! `https://openrouter.ai/api/v1/responses`. OpenRouter is a unified
//! gateway in front of hundreds of models from many vendors; the model
//! slug selects which one (e.g. `anthropic/claude-sonnet-4`,
//! `openai/gpt-4o`, `google/gemini-2.5-pro`).
//!
//! The OpenRouter Responses API is **stateless** — no conversation
//! state is persisted server-side and there is no `previous_response_id`
//! round-trip — so this builder always disables response-id reuse and
//! sends the full conversation each turn. Streaming is SSE over HTTP;
//! OpenRouter has no WebSocket/realtime endpoint, but the builder stays
//! generic over [`Transport`] so a custom transport can still be
//! supplied via [`OpenRouterBuilder::with_transport`].
//!
//! ```no_run
//! use chat_openrouter::OpenRouterBuilder;
//!
//! // OPENROUTER_API_KEY env var is read automatically.
//! let client = OpenRouterBuilder::new()
//!     .with_model("anthropic/claude-sonnet-4")
//!     .build();
//! ```

mod client;

use std::env;
use std::marker::PhantomData;

use chat_core::transport::Transport;
use chat_core::types::provider_meta::ProviderMeta;
use chat_responses::ResponsesBuilder;

pub use crate::client::OpenRouterClient;
pub use chat_core::transport::ReqwestTransport;

/// Default OpenRouter base URL. The Responses API lives at
/// `{base_url}/responses`.
pub const DEFAULT_OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

const OPENROUTER_API_KEY_ENV: &str = "OPENROUTER_API_KEY";

pub struct WithoutModel;
pub struct WithModel;

pub struct OpenRouterBuilder<M = WithoutModel, T: Transport = ReqwestTransport> {
    model_name: Option<String>,
    api_key: Option<String>,
    base_url: String,
    reasoning_effort: Option<String>,
    transport: Option<T>,
    meta: ProviderMeta,
    _m: PhantomData<M>,
}

impl Default for OpenRouterBuilder<WithoutModel, ReqwestTransport> {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenRouterBuilder<WithoutModel, ReqwestTransport> {
    pub fn new() -> Self {
        Self {
            model_name: None,
            api_key: None,
            base_url: DEFAULT_OPENROUTER_BASE_URL.to_string(),
            reasoning_effort: None,
            transport: Some(ReqwestTransport::default()),
            meta: ProviderMeta::default(),
            _m: PhantomData,
        }
    }
}

impl<M, T: Transport> OpenRouterBuilder<M, T> {
    /// Override the API key. If unset, `OPENROUTER_API_KEY` is read at build time.
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Override the base URL. Defaults to `https://openrouter.ai/api/v1`.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set the reasoning effort (`"low"` / `"medium"` / `"high"`) for
    /// reasoning-capable models. Forwarded to the Responses API as-is;
    /// models that don't support it ignore the field.
    pub fn with_reasoning_effort(mut self, effort: impl Into<String>) -> Self {
        self.reasoning_effort = Some(effort.into());
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

    /// Supply a custom transport, replacing the default `ReqwestTransport`.
    pub fn with_transport<T2: Transport>(self, transport: T2) -> OpenRouterBuilder<M, T2> {
        OpenRouterBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            base_url: self.base_url,
            reasoning_effort: self.reasoning_effort,
            transport: Some(transport),
            meta: self.meta,
            _m: PhantomData,
        }
    }
}

impl<T: Transport> OpenRouterBuilder<WithoutModel, T> {
    /// Select the model. OpenRouter model slugs are vendor-prefixed,
    /// e.g. `anthropic/claude-sonnet-4` or `openai/gpt-4o`.
    pub fn with_model(self, model: impl Into<String>) -> OpenRouterBuilder<WithModel, T> {
        OpenRouterBuilder {
            model_name: Some(model.into()),
            api_key: self.api_key,
            base_url: self.base_url,
            reasoning_effort: self.reasoning_effort,
            transport: self.transport,
            meta: self.meta,
            _m: PhantomData,
        }
    }
}

impl<T: Transport> OpenRouterBuilder<WithModel, T> {
    /// Build the client.
    ///
    /// Resolves the API key in this order: explicit `with_api_key()`,
    /// then the `OPENROUTER_API_KEY` env var. Panics if neither is
    /// present. Hands wire-level state to [`ResponsesBuilder`] with
    /// `previous_response_id` reuse disabled (the OpenRouter Responses
    /// API is stateless).
    pub fn build(self) -> OpenRouterClient<T> {
        let api_key = self
            .api_key
            .or_else(|| env::var(OPENROUTER_API_KEY_ENV).ok())
            .expect("No OpenRouter API key. Set OPENROUTER_API_KEY or call .with_api_key().");

        let transport = self.transport.expect("transport set");
        let model = self.model_name.expect("model set");

        let mut rb = ResponsesBuilder::new()
            .with_base_url(self.base_url)
            .with_model(model)
            .with_api_key(api_key)
            .with_transport(transport)
            .without_previous_response_id()
            .with_meta(self.meta);

        if let Some(eff) = self.reasoning_effort {
            rb = rb.with_reasoning_effort(eff);
        }

        OpenRouterClient { inner: rb.build() }
    }
}
