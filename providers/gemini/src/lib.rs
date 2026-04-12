mod api;
pub mod client;
mod tools;
use std::env;
use std::marker::PhantomData;

use crate::api::types::request::{
    EmbeddingsTask, GeminiEmbeddingsConfig, GeminiFunctionCallingConfig,
};
use chat_core::transport::Transport;
use chat_core::types::provider_meta::ProviderMeta;

pub use crate::client::GeminiClient;
use crate::tools::GeminiNativeTool;
use crate::tools::code_execution::CodeExecutionTool;
use crate::tools::google_maps::GoogleMapsTool;
use crate::tools::google_search::GoogleSearchTool;

pub use transport_reqwest::ReqwestTransport;

pub struct WithoutModel;
pub struct WithModel;

pub struct BaseConfig;
pub struct CompletionConfig;
pub struct EmbeddingConfig;

pub struct GeminiBuilder<M = WithoutModel, C = BaseConfig, T: Transport = ReqwestTransport> {
    model_name: Option<String>,
    api_key: Option<String>,
    native_tools: Vec<Box<dyn GeminiNativeTool>>,
    function_config: Option<GeminiFunctionCallingConfig>,
    embeddings_config: Option<GeminiEmbeddingsConfig>,
    include_thoughts: bool,
    transport: Option<T>,
    meta: ProviderMeta,
    _m: PhantomData<M>,
    _c: PhantomData<C>,
}

impl Default for GeminiBuilder<WithoutModel, BaseConfig, ReqwestTransport> {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiBuilder<WithoutModel, BaseConfig, ReqwestTransport> {
    pub fn new() -> Self {
        Self {
            model_name: None,
            api_key: None,
            native_tools: Vec::new(),
            function_config: None,
            embeddings_config: None,
            include_thoughts: false,
            transport: Some(ReqwestTransport::default()),
            meta: ProviderMeta::default(),
            _m: PhantomData,
            _c: PhantomData,
        }
    }
}

impl<M, C, T: Transport> GeminiBuilder<M, C, T> {
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
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
    pub fn with_transport<T2: Transport>(self, transport: T2) -> GeminiBuilder<M, C, T2> {
        GeminiBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            native_tools: self.native_tools,
            function_config: self.function_config,
            embeddings_config: self.embeddings_config,
            include_thoughts: self.include_thoughts,
            transport: Some(transport),
            meta: self.meta,
            _m: PhantomData,
            _c: PhantomData,
        }
    }
}

impl<C, T: Transport> GeminiBuilder<WithoutModel, C, T> {
    pub fn with_model(self, model_name: String) -> GeminiBuilder<WithModel, C, T> {
        GeminiBuilder {
            model_name: Some(model_name),
            api_key: self.api_key,
            native_tools: self.native_tools,
            function_config: self.function_config,
            embeddings_config: self.embeddings_config,
            include_thoughts: self.include_thoughts,
            transport: self.transport,
            meta: self.meta,
            _m: PhantomData,
            _c: PhantomData,
        }
    }
}

impl<M, T: Transport> GeminiBuilder<M, BaseConfig, T> {
    fn into_completion(self) -> GeminiBuilder<M, CompletionConfig, T> {
        GeminiBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            native_tools: self.native_tools,
            function_config: self.function_config,
            embeddings_config: self.embeddings_config,
            include_thoughts: self.include_thoughts,
            transport: self.transport,
            meta: self.meta,
            _m: PhantomData,
            _c: PhantomData,
        }
    }

    pub fn with_code_execution(self) -> GeminiBuilder<M, CompletionConfig, T> {
        self.into_completion().with_code_execution()
    }

    pub fn with_google_search(self) -> GeminiBuilder<M, CompletionConfig, T> {
        self.into_completion().with_google_search()
    }

    pub fn with_thoughts(mut self, include: bool) -> Self {
        self.include_thoughts = include;
        self
    }

    pub fn with_google_maps(
        self,
        lat_lng: Option<(f32, f32)>,
        widget: bool,
    ) -> GeminiBuilder<M, CompletionConfig, T> {
        self.into_completion().with_google_maps(lat_lng, widget)
    }

    pub fn with_function_calling_mode(
        self,
        mode: &str,
        allowed: Option<Vec<String>>,
    ) -> GeminiBuilder<M, CompletionConfig, T> {
        self.into_completion()
            .with_function_calling_mode(mode, allowed)
    }
}

impl<M, T: Transport> GeminiBuilder<M, CompletionConfig, T> {
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

impl<M, T: Transport> GeminiBuilder<M, BaseConfig, T> {
    fn into_embedding(self) -> GeminiBuilder<M, EmbeddingConfig, T> {
        GeminiBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            native_tools: vec![],
            function_config: None,
            embeddings_config: self.embeddings_config,
            include_thoughts: false,
            transport: self.transport,
            meta: self.meta,
            _m: PhantomData,
            _c: PhantomData,
        }
    }

    pub fn with_embeddings(self, dimensions: Option<usize>) -> GeminiBuilder<M, EmbeddingConfig, T> {
        self.into_embedding().with_embeddings(dimensions)
    }
}

impl<M, T: Transport> GeminiBuilder<M, EmbeddingConfig, T> {
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

impl<C, T: Transport> GeminiBuilder<WithModel, C, T> {
    pub fn build(self) -> GeminiClient<T> {
        GeminiClient {
            model_name: self.model_name.unwrap(),
            api_key: self.api_key.unwrap_or_else(|| {
                env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY not found in environment")
            }),
            host: "generativelanguage.googleapis.com".to_string(),
            base_path: "/v1beta".to_string(),
            transport: self.transport.expect(
                "No transport provided. Call .with_transport() or use the default GeminiBuilder (which provides ReqwestTransport).",
            ),
            native_tools: self.native_tools,
            function_config: self.function_config,
            embeddings_config: self.embeddings_config,
            include_thoughts: self.include_thoughts,
            meta: self.meta,
        }
    }
}
