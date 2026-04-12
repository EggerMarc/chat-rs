use chat_core::transport::Transport;
use chat_core::types::provider_meta::ProviderMeta;

use crate::tools::OpenAINativeTool;

pub struct OpenAIClient<T: Transport> {
    pub(crate) model_name: String,
    pub(crate) api_key: String,
    pub(crate) scheme: String,
    pub(crate) host: String,
    pub(crate) base_path: String,
    pub(crate) native_tools: Vec<Box<dyn OpenAINativeTool>>,
    pub(crate) transport: T,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) use_previous_response_id: bool,
    pub(crate) last_response_id: Option<String>,
    pub(crate) store: Option<bool>,
    pub(crate) meta: ProviderMeta,
}
