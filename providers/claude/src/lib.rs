mod api;
pub mod client;

use std::env;
use std::marker::PhantomData;

use chat_core::transport::Transport;
use chat_core::types::provider_meta::ProviderMeta;

pub use crate::client::ClaudeClient;
pub use transport_reqwest::ReqwestTransport;

const DEFAULT_API_VERSION: &str = "2023-06-01";
const DEFAULT_THINKING_BUDGET: u32 = 10000;

pub struct WithoutModel;
pub struct WithModel;

pub struct ClaudeBuilder<M = WithoutModel, T: Transport = ReqwestTransport> {
    model_name: Option<String>,
    api_key: Option<String>,
    api_version: Option<String>,
    include_thoughts: bool,
    thinking_budget: Option<u32>,
    transport: Option<T>,
    meta: ProviderMeta,
    _m: PhantomData<M>,
}

impl Default for ClaudeBuilder<WithoutModel, ReqwestTransport> {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeBuilder<WithoutModel, ReqwestTransport> {
    pub fn new() -> Self {
        Self {
            model_name: None,
            api_key: None,
            api_version: None,
            include_thoughts: true,
            thinking_budget: None,
            transport: Some(ReqwestTransport::default()),
            meta: ProviderMeta::default(),
            _m: PhantomData,
        }
    }
}

impl<M, T: Transport> ClaudeBuilder<M, T> {
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    pub fn with_api_version(mut self, version: String) -> Self {
        self.api_version = Some(version);
        self
    }

    pub fn with_thoughts(mut self, enabled: bool) -> Self {
        self.include_thoughts = enabled;
        self
    }

    pub fn with_thinking_budget(mut self, budget: u32) -> Self {
        self.thinking_budget = Some(budget);
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
    pub fn with_transport<T2: Transport>(self, transport: T2) -> ClaudeBuilder<M, T2> {
        ClaudeBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            api_version: self.api_version,
            include_thoughts: self.include_thoughts,
            thinking_budget: self.thinking_budget,
            transport: Some(transport),
            meta: self.meta,
            _m: PhantomData,
        }
    }
}

impl<T: Transport> ClaudeBuilder<WithoutModel, T> {
    pub fn with_model(self, model_name: String) -> ClaudeBuilder<WithModel, T> {
        ClaudeBuilder {
            model_name: Some(model_name),
            api_key: self.api_key,
            api_version: self.api_version,
            include_thoughts: self.include_thoughts,
            thinking_budget: self.thinking_budget,
            transport: self.transport,
            meta: self.meta,
            _m: PhantomData,
        }
    }
}

impl<T: Transport> ClaudeBuilder<WithModel, T> {
    pub fn build(self) -> ClaudeClient<T> {
        ClaudeClient {
            model_name: self.model_name.unwrap(),
            api_key: self.api_key.unwrap_or_else(|| {
                env::var("CLAUDE_API_KEY").expect("CLAUDE_API_KEY not found in environment")
            }),
            api_version: self
                .api_version
                .unwrap_or_else(|| DEFAULT_API_VERSION.to_string()),
            transport: self.transport.expect(
                "No transport provided. Call .with_transport() or use the default ClaudeBuilder (which provides ReqwestTransport).",
            ),
            include_thoughts: self.include_thoughts,
            thinking_budget: if self.include_thoughts {
                Some(self.thinking_budget.unwrap_or(DEFAULT_THINKING_BUDGET))
            } else {
                None
            },
            meta: self.meta,
        }
    }
}
