use chat_core::transport::Transport;
use chat_core::types::provider_meta::ProviderMeta;

pub struct ClaudeClient<T: Transport> {
    pub(crate) model_name: String,
    pub(crate) api_key: String,
    pub(crate) api_version: String,
    pub(crate) transport: T,
    pub(crate) include_thoughts: bool,
    pub(crate) thinking_budget: Option<u32>,
    pub(crate) meta: ProviderMeta,
}
