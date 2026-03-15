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

pub struct BaseConfig;
pub struct CompletionConfig;
pub struct EmbeddingConfig;

pub struct OpenAIBuilder<M = WithoutModel, U = BaseEndpoint, C = BaseConfig> {
    model_name: Option<String>,
    api_key: Option<String>,
    base_url: String,
    native_tools: Vec<Box<dyn OpenAINativeTool>>,
    reasoning_effort: Option<String>,
    _m: PhantomData<M>,
    _u: PhantomData<U>,
    _c: PhantomData<C>,
}

impl Default for OpenAIBuilder<WithoutModel, BaseEndpoint, BaseConfig> {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAIBuilder<WithoutModel, BaseEndpoint, BaseConfig> {
    pub fn new() -> Self {
        Self {
            model_name: None,
            api_key: None,
            base_url: "https://api.openai.com/v1".to_string(),
            native_tools: Vec::new(),
            reasoning_effort: None,
            _m: PhantomData,
            _u: PhantomData,
            _c: PhantomData,
        }
    }
}

impl<U, C> OpenAIBuilder<WithoutModel, U, C> {
    pub fn with_model(self, model_name: impl Into<String>) -> OpenAIBuilder<WithModel, U, C> {
        OpenAIBuilder {
            model_name: Some(model_name.into()),
            api_key: self.api_key,
            base_url: self.base_url,
            native_tools: self.native_tools,
            reasoning_effort: self.reasoning_effort,
            _m: PhantomData,
            _u: PhantomData,
            _c: PhantomData,
        }
    }
}

impl<M, U, C> OpenAIBuilder<M, U, C> {
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }
}

impl<M, C> OpenAIBuilder<M, BaseEndpoint, C> {
    pub fn with_custom_url(self, base_url: String) -> OpenAIBuilder<M, CustomEndpoint, C> {
        OpenAIBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            native_tools: self.native_tools,
            reasoning_effort: self.reasoning_effort,
            _m: PhantomData,
            _u: PhantomData,
            _c: PhantomData,
        }
    }
}

impl<M> OpenAIBuilder<M, BaseEndpoint, BaseConfig> {
    fn into_completion(self) -> OpenAIBuilder<M, BaseEndpoint, CompletionConfig> {
        OpenAIBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            base_url: self.base_url,
            native_tools: self.native_tools,
            reasoning_effort: self.reasoning_effort,
            _m: PhantomData,
            _u: PhantomData,
            _c: PhantomData,
        }
    }

    pub fn with_reasoning_effort(
        self,
        effort: &str,
    ) -> OpenAIBuilder<M, BaseEndpoint, CompletionConfig> {
        self.into_completion().with_reasoning_effort(effort)
    }

    pub fn with_web_search(
        self,
        context_size: Option<SearchContextSizeEnum>,
        user_location: Option<UserLocation>,
    ) -> OpenAIBuilder<M, BaseEndpoint, CompletionConfig> {
        self.into_completion()
            .with_web_search(context_size, user_location)
    }
}

impl<M> OpenAIBuilder<M, CustomEndpoint, BaseConfig> {
    fn into_completion(self) -> OpenAIBuilder<M, CustomEndpoint, CompletionConfig> {
        OpenAIBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            base_url: self.base_url,
            native_tools: self.native_tools,
            reasoning_effort: self.reasoning_effort,
            _m: PhantomData,
            _u: PhantomData,
            _c: PhantomData,
        }
    }

    pub fn with_reasoning_effort(
        self,
        effort: &str,
    ) -> OpenAIBuilder<M, CustomEndpoint, CompletionConfig> {
        self.into_completion().with_reasoning_effort(effort)
    }

    pub fn with_web_search(
        self,
        context_size: Option<SearchContextSizeEnum>,
        user_location: Option<UserLocation>,
    ) -> OpenAIBuilder<M, CustomEndpoint, CompletionConfig> {
        self.into_completion()
            .with_web_search(context_size, user_location)
    }
}

impl<M> OpenAIBuilder<M, BaseEndpoint, CompletionConfig> {
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

impl<M> OpenAIBuilder<M, CustomEndpoint, CompletionConfig> {
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

impl<M, U> OpenAIBuilder<M, U, BaseConfig> {
    fn into_embedding(self) -> OpenAIBuilder<M, U, EmbeddingConfig> {
        OpenAIBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            base_url: self.base_url,
            native_tools: vec![],
            reasoning_effort: None,
            _m: PhantomData,
            _u: PhantomData,
            _c: PhantomData,
        }
    }

    pub fn with_embeddings(self) -> OpenAIBuilder<M, U, EmbeddingConfig> {
        self.into_embedding()
    }
}

impl<U, C> OpenAIBuilder<WithModel, U, C> {
    pub fn build(self) -> OpenAIClient {
        let api_key = self
            .api_key
            .or_else(|| env::var("OPENAI_API_KEY").ok())
            .expect("No api key found");

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
