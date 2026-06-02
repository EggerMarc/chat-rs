//! OpenRouter provider for chat-rs.
//!
//! OpenRouter is a unified gateway in front of hundreds of models from
//! many vendors; the model slug selects which one (e.g.
//! `anthropic/claude-sonnet-4`, `openai/gpt-4o`, `google/gemini-2.5-pro`).
//!
//! It exposes two OpenAI-compatible wires, and the builder picks one
//! upstream and hands off to the matching wire crate — no wrapper type:
//!
//! - **Responses API (Beta)** (default) — wraps [`chat_responses`] and
//!   builds a [`ResponsesClient`]. Stateless (no `previous_response_id`
//!   round-trip), so response-id reuse is disabled and the full
//!   conversation is sent each turn.
//! - **Chat Completions** — opt in with [`OpenRouterBuilder::with_completions`];
//!   wraps [`chat_completions`] and builds a [`ChatCompletionsClient`].
//!
//! Both wire clients already implement `CompletionProvider` (and
//! `StreamProvider` under the `stream` feature), so streaming is free
//! on either path. OpenRouter has no WebSocket endpoint — streaming is
//! SSE over HTTP — but the builder stays generic over [`Transport`].
//!
//! ```no_run
//! use chat_openrouter::OpenRouterBuilder;
//!
//! // OPENROUTER_API_KEY env var is read automatically.
//! // Default: Responses API.
//! let responses = OpenRouterBuilder::new()
//!     .with_model("anthropic/claude-sonnet-4")
//!     .build();
//!
//! // Opt in to the Chat Completions API.
//! let completions = OpenRouterBuilder::new()
//!     .with_completions()
//!     .with_model("openai/gpt-4o")
//!     .build();
//! ```

use std::env;
use std::marker::PhantomData;

pub use chat_completions::{ChatCompletionsClient, ReqwestTransport};
use chat_completions::ChatCompletionsBuilder;
use chat_core::transport::Transport;
pub use chat_responses::ResponsesClient;
use chat_responses::ResponsesBuilder;

/// Default OpenRouter base URL. The Responses API lives at
/// `{base_url}/responses`, Chat Completions at `{base_url}/chat/completions`.
pub const DEFAULT_OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

const OPENROUTER_API_KEY_ENV: &str = "OPENROUTER_API_KEY";

pub struct WithoutModel;
pub struct WithModel;

/// Wire type-state: target the Responses API (default).
pub struct Responses;
/// Wire type-state: target the Chat Completions API.
pub struct Completions;

pub struct OpenRouterBuilder<M = WithoutModel, W = Responses, T: Transport = ReqwestTransport> {
    model: Option<String>,
    api_key: Option<String>,
    base_url: String,
    reasoning_effort: Option<String>,
    description: Option<String>,
    transport: Option<T>,
    _m: PhantomData<M>,
    _w: PhantomData<W>,
}

impl Default for OpenRouterBuilder<WithoutModel, Responses, ReqwestTransport> {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenRouterBuilder<WithoutModel, Responses, ReqwestTransport> {
    pub fn new() -> Self {
        Self {
            model: None,
            api_key: None,
            base_url: DEFAULT_OPENROUTER_BASE_URL.to_string(),
            reasoning_effort: None,
            description: None,
            transport: Some(ReqwestTransport::default()),
            _m: PhantomData,
            _w: PhantomData,
        }
    }
}

impl<M, W, T: Transport> OpenRouterBuilder<M, W, T> {
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

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Supply a custom transport, replacing the default `ReqwestTransport`.
    pub fn with_transport<T2: Transport>(self, transport: T2) -> OpenRouterBuilder<M, W, T2> {
        OpenRouterBuilder {
            model: self.model,
            api_key: self.api_key,
            base_url: self.base_url,
            reasoning_effort: self.reasoning_effort,
            description: self.description,
            transport: Some(transport),
            _m: PhantomData,
            _w: PhantomData,
        }
    }
}

impl<W, T: Transport> OpenRouterBuilder<WithoutModel, W, T> {
    /// Select the model. OpenRouter slugs are vendor-prefixed, e.g.
    /// `anthropic/claude-sonnet-4` or `openai/gpt-4o`.
    pub fn with_model(self, model: impl Into<String>) -> OpenRouterBuilder<WithModel, W, T> {
        OpenRouterBuilder {
            model: Some(model.into()),
            api_key: self.api_key,
            base_url: self.base_url,
            reasoning_effort: self.reasoning_effort,
            description: self.description,
            transport: self.transport,
            _m: PhantomData,
            _w: PhantomData,
        }
    }
}

impl<M, T: Transport> OpenRouterBuilder<M, Responses, T> {
    /// Opt in to the Chat Completions wire instead of the default Responses API.
    pub fn with_completions(self) -> OpenRouterBuilder<M, Completions, T> {
        OpenRouterBuilder {
            model: self.model,
            api_key: self.api_key,
            base_url: self.base_url,
            reasoning_effort: self.reasoning_effort,
            description: self.description,
            transport: self.transport,
            _m: PhantomData,
            _w: PhantomData,
        }
    }

    /// Set the reasoning effort (`"low"` / `"medium"` / `"high"`) for
    /// reasoning-capable models. Responses-wire only — Chat Completions
    /// does not carry this field.
    pub fn with_reasoning_effort(mut self, effort: impl Into<String>) -> Self {
        self.reasoning_effort = Some(effort.into());
        self
    }
}

impl<M, W, T: Transport> OpenRouterBuilder<M, W, T> {
    fn resolve_api_key(&mut self) -> String {
        self.api_key
            .take()
            .or_else(|| env::var(OPENROUTER_API_KEY_ENV).ok())
            .expect("No OpenRouter API key. Set OPENROUTER_API_KEY or call .with_api_key().")
    }
}

impl<T: Transport> OpenRouterBuilder<WithModel, Responses, T> {
    /// Build a Responses API client.
    pub fn build(mut self) -> ResponsesClient<T> {
        let api_key = self.resolve_api_key();
        let mut rb = ResponsesBuilder::new()
            .with_base_url(self.base_url)
            .with_model(self.model.expect("model set"))
            .with_api_key(api_key)
            .with_transport(self.transport.expect("transport set"))
            .without_previous_response_id();

        if let Some(eff) = self.reasoning_effort {
            rb = rb.with_reasoning_effort(eff);
        }
        if let Some(desc) = self.description {
            rb = rb.with_description(desc);
        }
        rb.build()
    }
}

impl<T: Transport> OpenRouterBuilder<WithModel, Completions, T> {
    /// Build a Chat Completions client.
    pub fn build(mut self) -> ChatCompletionsClient<T> {
        let api_key = self.resolve_api_key();
        let mut cb = ChatCompletionsBuilder::new()
            .with_base_url(self.base_url)
            .with_model(self.model.expect("model set"))
            .with_api_key(api_key)
            .with_transport(self.transport.expect("transport set"));

        if let Some(desc) = self.description {
            cb = cb.with_description(desc);
        }
        cb.build()
    }
}
