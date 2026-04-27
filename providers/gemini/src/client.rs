use chat_core::transport::Transport;
use chat_core::types::provider_meta::ProviderMeta;

use crate::{
    api::types::request::{GeminiEmbeddingsConfig, GeminiFunctionCallingConfig},
    tools::GeminiNativeTool,
};

pub struct GeminiClient<T: Transport> {
    pub(crate) model_name: String,
    pub(crate) api_key: String,
    pub(crate) scheme: String,
    pub(crate) host: String,
    pub(crate) base_path: String,
    pub(crate) transport: T,
    pub(crate) native_tools: Vec<Box<dyn GeminiNativeTool>>,
    pub(crate) function_config: Option<GeminiFunctionCallingConfig>,
    pub(crate) embeddings_config: Option<GeminiEmbeddingsConfig>,
    pub(crate) include_thoughts: bool,
    pub(crate) response_modalities: Option<Vec<String>>,
    pub(crate) meta: ProviderMeta,
}
