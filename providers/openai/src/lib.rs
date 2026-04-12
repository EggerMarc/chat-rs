mod api;
mod client;
mod tools;
use std::env;
use std::marker::PhantomData;

use chat_core::transport::Transport;
use chat_core::types::provider_meta::ProviderMeta;

pub use crate::client::OpenAIClient;
use crate::tools::{
    OpenAINativeTool, web_search::SearchContextSizeEnum, web_search::UserLocation,
    web_search::WebSearchTool,
};

pub use transport_reqwest::ReqwestTransport;

pub struct WithoutModel;
pub struct WithModel;

pub struct BaseEndpoint;
pub struct CustomEndpoint;

pub struct BaseConfig;
pub struct CompletionConfig;
pub struct EmbeddingConfig;

pub struct OpenAIBuilder<M = WithoutModel, U = BaseEndpoint, C = BaseConfig, T: Transport = ReqwestTransport> {
    model_name: Option<String>,
    api_key: Option<String>,
    base_url: String,
    native_tools: Vec<Box<dyn OpenAINativeTool>>,
    reasoning_effort: Option<String>,
    use_previous_response_id: bool,
    store: Option<bool>,
    transport: Option<T>,
    meta: ProviderMeta,
    _m: PhantomData<M>,
    _u: PhantomData<U>,
    _c: PhantomData<C>,
}

impl Default for OpenAIBuilder<WithoutModel, BaseEndpoint, BaseConfig, ReqwestTransport> {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenAIBuilder<WithoutModel, BaseEndpoint, BaseConfig, ReqwestTransport> {
    pub fn new() -> Self {
        Self {
            model_name: None,
            api_key: None,
            base_url: "https://api.openai.com/v1".to_string(),
            native_tools: Vec::new(),
            reasoning_effort: None,
            use_previous_response_id: true,
            store: None,
            transport: Some(ReqwestTransport::default()),
            meta: ProviderMeta::default(),
            _m: PhantomData,
            _u: PhantomData,
            _c: PhantomData,
        }
    }
}

impl<U, C, T: Transport> OpenAIBuilder<WithoutModel, U, C, T> {
    pub fn with_model(self, model_name: impl Into<String>) -> OpenAIBuilder<WithModel, U, C, T> {
        OpenAIBuilder {
            model_name: Some(model_name.into()),
            api_key: self.api_key,
            base_url: self.base_url,
            native_tools: self.native_tools,
            reasoning_effort: self.reasoning_effort,
            use_previous_response_id: self.use_previous_response_id,
            store: self.store,
            transport: self.transport,
            meta: self.meta,
            _m: PhantomData,
            _u: PhantomData,
            _c: PhantomData,
        }
    }
}

impl<M, U, C, T: Transport> OpenAIBuilder<M, U, C, T> {
    pub fn without_previous_response_id(mut self) -> Self {
        self.use_previous_response_id = false;
        self
    }

    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    pub fn with_store(mut self, store: bool) -> Self {
        self.store = Some(store);
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
    pub fn with_transport<T2: Transport>(self, transport: T2) -> OpenAIBuilder<M, U, C, T2> {
        OpenAIBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            base_url: self.base_url,
            native_tools: self.native_tools,
            reasoning_effort: self.reasoning_effort,
            use_previous_response_id: self.use_previous_response_id,
            store: self.store,
            transport: Some(transport),
            meta: self.meta,
            _m: PhantomData,
            _u: PhantomData,
            _c: PhantomData,
        }
    }
}

impl<M, C, T: Transport> OpenAIBuilder<M, BaseEndpoint, C, T> {
    pub fn with_custom_url(self, base_url: String) -> OpenAIBuilder<M, CustomEndpoint, C, T> {
        OpenAIBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            base_url: base_url.trim_end_matches('/').to_string(),
            native_tools: self.native_tools,
            reasoning_effort: self.reasoning_effort,
            use_previous_response_id: self.use_previous_response_id,
            store: self.store,
            transport: self.transport,
            meta: self.meta,
            _m: PhantomData,
            _u: PhantomData,
            _c: PhantomData,
        }
    }
}

impl<M, T: Transport> OpenAIBuilder<M, BaseEndpoint, BaseConfig, T> {
    fn into_completion(self) -> OpenAIBuilder<M, BaseEndpoint, CompletionConfig, T> {
        OpenAIBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            base_url: self.base_url,
            native_tools: self.native_tools,
            reasoning_effort: self.reasoning_effort,
            use_previous_response_id: self.use_previous_response_id,
            store: self.store,
            transport: self.transport,
            meta: self.meta,
            _m: PhantomData,
            _u: PhantomData,
            _c: PhantomData,
        }
    }

    pub fn with_reasoning_effort(
        self,
        effort: &str,
    ) -> OpenAIBuilder<M, BaseEndpoint, CompletionConfig, T> {
        self.into_completion().with_reasoning_effort(effort)
    }

    pub fn with_web_search(
        self,
        context_size: Option<SearchContextSizeEnum>,
        user_location: Option<UserLocation>,
    ) -> OpenAIBuilder<M, BaseEndpoint, CompletionConfig, T> {
        self.into_completion()
            .with_web_search(context_size, user_location)
    }
}

impl<M, T: Transport> OpenAIBuilder<M, CustomEndpoint, BaseConfig, T> {
    fn into_completion(self) -> OpenAIBuilder<M, CustomEndpoint, CompletionConfig, T> {
        OpenAIBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            base_url: self.base_url,
            native_tools: self.native_tools,
            reasoning_effort: self.reasoning_effort,
            use_previous_response_id: self.use_previous_response_id,
            store: self.store,
            transport: self.transport,
            meta: self.meta,
            _m: PhantomData,
            _u: PhantomData,
            _c: PhantomData,
        }
    }

    pub fn with_reasoning_effort(
        self,
        effort: &str,
    ) -> OpenAIBuilder<M, CustomEndpoint, CompletionConfig, T> {
        self.into_completion().with_reasoning_effort(effort)
    }

    pub fn with_web_search(
        self,
        context_size: Option<SearchContextSizeEnum>,
        user_location: Option<UserLocation>,
    ) -> OpenAIBuilder<M, CustomEndpoint, CompletionConfig, T> {
        self.into_completion()
            .with_web_search(context_size, user_location)
    }
}

impl<M, T: Transport> OpenAIBuilder<M, BaseEndpoint, CompletionConfig, T> {
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

impl<M, T: Transport> OpenAIBuilder<M, CustomEndpoint, CompletionConfig, T> {
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

impl<M, U, T: Transport> OpenAIBuilder<M, U, BaseConfig, T> {
    fn into_embedding(self) -> OpenAIBuilder<M, U, EmbeddingConfig, T> {
        OpenAIBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            base_url: self.base_url,
            native_tools: vec![],
            reasoning_effort: None,
            use_previous_response_id: false,
            store: None,
            transport: self.transport,
            meta: self.meta,
            _m: PhantomData,
            _u: PhantomData,
            _c: PhantomData,
        }
    }

    pub fn with_embeddings(self) -> OpenAIBuilder<M, U, EmbeddingConfig, T> {
        self.into_embedding()
    }
}

impl<U, C, T: Transport> OpenAIBuilder<WithModel, U, C, T> {
    /// Build the client.
    ///
    /// Panics if no transport was provided via `.with_transport()` and
    /// `T` is not the default `ReqwestTransport`.
    pub fn build(self) -> OpenAIClient<T> {
        let api_key = self
            .api_key
            .or_else(|| env::var("OPENAI_API_KEY").ok())
            .expect("No api key found");

        let transport = self.transport.expect(
            "No transport provided. Call .with_transport() or use the default OpenAIBuilder (which provides ReqwestTransport).",
        );

        OpenAIClient {
            model_name: self.model_name.unwrap(),
            api_key,
            base_url: self.base_url,
            transport,
            native_tools: self.native_tools,
            reasoning_effort: self.reasoning_effort,
            use_previous_response_id: self.use_previous_response_id,
            last_response_id: None,
            store: self.store,
            meta: self.meta,
        }
    }
}

