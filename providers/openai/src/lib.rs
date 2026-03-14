mod api;
mod client;
mod tools;
use std::env;
use std::marker::PhantomData;

use crate::client::OpenAIClient;
use crate::tools::{
    OpenAINativeTool, web_search::SearchContextSizeEnum, web_search::UserLocation,
    web_search::WebSearchTool,
};

pub struct WithoutModel;
pub struct WithModel;

pub struct BaseEndpoint;
pub struct CustomEndpoint;

pub struct OpenAIBuilder<M = WithoutModel, U = BaseEndpoint> {
    model_name: Option<String>,
    api_key: Option<String>,
    base_url: String,
    native_tools: Vec<Box<dyn OpenAINativeTool>>,
    reasoning_effort: Option<String>,
    _m: PhantomData<M>,
    _u: PhantomData<U>,
}

impl Default for OpenAIBuilder<WithoutModel, BaseEndpoint> {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAIBuilder<WithoutModel, BaseEndpoint> {
    pub fn new() -> Self {
        Self {
            model_name: None,
            api_key: None,
            base_url: "https://api.openai.com/v1".to_string(),
            native_tools: Vec::new(),
            reasoning_effort: None,
            _m: PhantomData,
            _u: PhantomData,
        }
    }
}

impl<U> OpenAIBuilder<WithoutModel, U> {
    pub fn with_model(self, model_name: String) -> OpenAIBuilder<WithModel, U> {
        OpenAIBuilder {
            model_name: Some(model_name),
            api_key: self.api_key,
            base_url: self.base_url,
            native_tools: self.native_tools,
            reasoning_effort: self.reasoning_effort,
            _m: PhantomData,
            _u: PhantomData,
        }
    }
}

impl<M, U> OpenAIBuilder<M, U> {
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }
}

impl<M> OpenAIBuilder<M, BaseEndpoint> {
    pub fn with_custom_url(self, base_url: String) -> OpenAIBuilder<M, CustomEndpoint> {
        OpenAIBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            native_tools: self.native_tools,
            reasoning_effort: self.reasoning_effort,
            _m: PhantomData,
            _u: PhantomData,
        }
    }
}

impl<M> OpenAIBuilder<M, BaseEndpoint> {
    pub fn with_reasoning_effort(mut self, effort: &str) -> Self {
        self.reasoning_effort = Some(effort.to_string());
        self
    }

    pub fn with_web_search(
        mut self,
        context_size: Option<SearchContextSizeEnum>,
        user_location: Option<UserLocation>,
    ) -> Self {
        self.native_tools.push(Box::new(WebSearchTool {
            context_size,
            user_location,
        }));
        self
    }
}

impl<M> OpenAIBuilder<M, CustomEndpoint> {
    pub fn with_reasoning_effort(mut self, effort: &str) -> Self {
        eprintln!(
            "WARNING: 'reasoning_effort' is an OpenAI-specific feature (e.g. for o1/o3 models). Custom endpoints (like Ollama) may reject this request."
        );
        self.reasoning_effort = Some(effort.to_string());
        self
    }

    pub fn with_web_search(
        mut self,
        context_size: Option<SearchContextSizeEnum>,
        user_location: Option<UserLocation>,
    ) -> Self {
        eprintln!(
            "WARNING: 'web_search' is an OpenAI-specific native tool. Custom endpoints may reject this request."
        );
        self.native_tools.push(Box::new(WebSearchTool {
            context_size,
            user_location,
        }));
        self
    }
}

impl<U> OpenAIBuilder<WithModel, U> {
    pub fn build(self) -> OpenAIClient {
        let api_key = self
            .api_key
            .or_else(|| env::var("OPENAI_API_KEY").ok())
            .unwrap_or_else(|| "dummy".to_string());

        OpenAIClient {
            model_name: self.model_name.unwrap(),
            api_key,
            base_url: self.base_url,
            http_client: reqwest::Client::new(),
            native_tools: self.native_tools,
            reasoning_effort: self.reasoning_effort,
        }
    }
}
