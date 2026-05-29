use chat_core::transport::Transport;
use chat_core::types::provider_meta::ProviderMeta;
use serde_json::Value;

/// Generic client over the OpenAI Responses API wire format
/// (`POST {base_url}/responses`). Provider wrappers like
/// [`chat_openai`] hold one of these as their inner state.
pub struct ResponsesClient<T: Transport> {
    pub model_name: String,
    pub api_key: String,
    pub scheme: String,
    pub host: String,
    pub base_path: String,
    pub transport: T,
    /// Tool declarations injected by the wrapping provider on top of the
    /// `ToolDeclarations` view supplied by `Chat`. The wire layer drops
    /// these into `tools[]` opaquely — it does not know about
    /// `OpenAINativeTool` or any provider-specific trait.
    pub extra_tool_declarations: Vec<Value>,
    pub reasoning_effort: Option<String>,
    pub use_previous_response_id: bool,
    pub(crate) last_response_id: Option<String>,
    pub store: Option<bool>,
    pub meta: ProviderMeta,
}
