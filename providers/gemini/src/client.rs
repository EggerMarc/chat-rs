use reqwest::Client;

use crate::{
    api::types::request::{GeminiEmbeddingsConfig, GeminiFunctionCallingConfig},
    tools::GeminiNativeTool,
};

pub struct GeminiClient {
    pub(crate) model_name: String,
    pub(crate) api_key: String,
    pub(crate) http_client: Client,
    pub(crate) native_tools: Vec<Box<dyn GeminiNativeTool>>,
    pub(crate) function_config: Option<GeminiFunctionCallingConfig>,
    pub(crate) embeddings_config: Option<GeminiEmbeddingsConfig>,
    pub(crate) include_thoughts: bool,
}
