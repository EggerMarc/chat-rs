mod api;
pub mod client;
mod tools;
use std::env;
use std::marker::PhantomData;

use crate::api::types::request::{
    EmbeddingsTask, GeminiEmbeddingsConfig, GeminiFunctionCallingConfig,
};
use crate::client::GeminiClient;
use crate::tools::code_execution::CodeExecutionTool;
use crate::tools::google_maps::GoogleMapsTool;
use crate::tools::google_search::GoogleSearchTool;
use crate::tools::GeminiNativeTool;

pub struct WithoutModel;
pub struct WithModel;

pub struct BaseConfig;
pub struct CompletionConfig;
pub struct EmbeddingConfig;

pub struct GeminiBuilder<M = WithoutModel, C = BaseConfig> {
    model_name: Option<String>,
    api_key: Option<String>,
    native_tools: Vec<Box<dyn GeminiNativeTool>>,
    function_config: Option<GeminiFunctionCallingConfig>,
    embeddings_config: Option<GeminiEmbeddingsConfig>,
    _m: PhantomData<M>,
    _c: PhantomData<C>,
}

impl Default for GeminiBuilder<WithoutModel, BaseConfig> {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiBuilder<WithoutModel, BaseConfig> {
    pub fn new() -> Self {
        Self {
            model_name: None,
            api_key: None,
            native_tools: Vec::new(),
            function_config: None,
            embeddings_config: None,
            _m: PhantomData,
            _c: PhantomData,
        }
    }
}

impl<M, C> GeminiBuilder<M, C> {
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }
}

impl<C> GeminiBuilder<WithoutModel, C> {
    pub fn with_model(self, model_name: String) -> GeminiBuilder<WithModel, C> {
        GeminiBuilder {
            model_name: Some(model_name),
            api_key: self.api_key,
            native_tools: self.native_tools,
            function_config: self.function_config,
            embeddings_config: self.embeddings_config,
            _m: PhantomData,
            _c: PhantomData,
        }
    }
}

impl<M> GeminiBuilder<M, BaseConfig> {
    fn into_completion(self) -> GeminiBuilder<M, CompletionConfig> {
        GeminiBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            native_tools: self.native_tools,
            function_config: self.function_config,
            embeddings_config: self.embeddings_config,
            _m: PhantomData,
            _c: PhantomData,
        }
    }

    pub fn with_code_execution(self) -> GeminiBuilder<M, CompletionConfig> {
        self.into_completion().with_code_execution()
    }

    pub fn with_google_search(self) -> GeminiBuilder<M, CompletionConfig> {
        self.into_completion().with_google_search()
    }
}

impl<M> GeminiBuilder<M, CompletionConfig> {
    pub fn with_code_execution(mut self) -> Self {
        self.native_tools.push(Box::new(CodeExecutionTool));
        self
    }

    pub fn with_google_search(mut self) -> Self {
        self.native_tools.push(Box::new(GoogleSearchTool {
            dynamic_threshold: None,
        }));
        self
    }

    pub fn with_google_search_threshold(mut self, threshold: f32) -> Self {
        self.native_tools.push(Box::new(GoogleSearchTool {
            dynamic_threshold: Some(threshold),
        }));
        self
    }

    pub fn with_google_maps(mut self, lat_lng: Option<(f32, f32)>, widget: bool) -> Self {
        self.native_tools.push(Box::new(GoogleMapsTool {
            lat_lng,
            enable_widget: widget,
        }));
        self
    }

    pub fn with_function_calling_mode(mut self, mode: &str, allowed: Option<Vec<String>>) -> Self {
        self.function_config = Some(GeminiFunctionCallingConfig {
            mode: mode.to_string(),
            allowed_function_names: allowed,
        });
        self
    }
}

impl<M> GeminiBuilder<M, BaseConfig> {
    fn into_embedding(self) -> GeminiBuilder<M, EmbeddingConfig> {
        GeminiBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            native_tools: self.native_tools,
            function_config: self.function_config,
            embeddings_config: self.embeddings_config,
            _m: PhantomData,
            _c: PhantomData,
        }
    }

    pub fn with_embeddings(self, dimensions: Option<usize>) -> GeminiBuilder<M, EmbeddingConfig> {
        self.into_embedding().with_embeddings(dimensions)
    }
}

impl<M> GeminiBuilder<M, EmbeddingConfig> {
    pub fn with_embeddings(mut self, dimensions: Option<usize>) -> Self {
        let mut config = self.embeddings_config.unwrap_or_default();
        config.dimensions = dimensions;
        self.embeddings_config = Some(config);
        self
    }

    pub fn with_embeddings_task(mut self, task: EmbeddingsTask) -> Self {
        let mut config = self.embeddings_config.unwrap_or_default();
        config.task = task;
        self.embeddings_config = Some(config);
        self
    }
}

impl<C> GeminiBuilder<WithModel, C> {
    pub fn build(self) -> GeminiClient {
        GeminiClient {
            model_name: self.model_name.unwrap(),
            api_key: self.api_key.unwrap_or_else(|| {
                env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY not found in environment")
            }),
            http_client: reqwest::Client::new(),
            native_tools: self.native_tools,
            function_config: self.function_config,
            embeddings_config: self.embeddings_config,
        }
    }
}

