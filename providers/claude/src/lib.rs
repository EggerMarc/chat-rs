mod api;
pub mod client;

use std::env;
use std::marker::PhantomData;

use crate::client::ClaudeClient;

const DEFAULT_API_VERSION: &str = "2023-06-01";
const DEFAULT_THINKING_BUDGET: u32 = 10000;

pub struct WithoutModel;
pub struct WithModel;

pub struct ClaudeBuilder<M = WithoutModel> {
    model_name: Option<String>,
    api_key: Option<String>,
    api_version: Option<String>,
    include_thoughts: bool,
    thinking_budget: Option<u32>,
    _m: PhantomData<M>,
}

impl Default for ClaudeBuilder<WithoutModel> {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeBuilder<WithoutModel> {
    pub fn new() -> Self {
        Self {
            model_name: None,
            api_key: None,
            api_version: None,
            include_thoughts: true,
            thinking_budget: None,
            _m: PhantomData,
        }
    }
}

impl<M> ClaudeBuilder<M> {
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
}

impl ClaudeBuilder<WithoutModel> {
    pub fn with_model(self, model_name: String) -> ClaudeBuilder<WithModel> {
        ClaudeBuilder {
            model_name: Some(model_name),
            api_key: self.api_key,
            api_version: self.api_version,
            include_thoughts: self.include_thoughts,
            thinking_budget: self.thinking_budget,
            _m: PhantomData,
        }
    }
}

impl ClaudeBuilder<WithModel> {
    pub fn build(self) -> ClaudeClient {
        ClaudeClient {
            model_name: self.model_name.unwrap(),
            api_key: self.api_key.unwrap_or_else(|| {
                env::var("CLAUDE_API_KEY").expect("CLAUDE_API_KEY not found in environment")
            }),
            api_version: self
                .api_version
                .unwrap_or_else(|| DEFAULT_API_VERSION.to_string()),
            http_client: reqwest::Client::new(),
            include_thoughts: self.include_thoughts,
            thinking_budget: if self.include_thoughts {
                Some(self.thinking_budget.unwrap_or(DEFAULT_THINKING_BUDGET))
            } else {
                None
            },
        }
    }
}
