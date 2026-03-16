use crate::tools::OpenAINativeTool;

pub struct OpenAIClient {
    pub(crate) model_name: String,
    pub(crate) api_key: String,
    pub(crate) base_url: String,
    pub(crate) native_tools: Vec<Box<dyn OpenAINativeTool>>,
    pub(crate) http_client: reqwest::Client,
    pub(crate) reasoning_effort: Option<String>,
    pub(crate) use_previous_response_id: bool,
    pub(crate) last_response_id: Option<String>,
    pub(crate) store: Option<bool>,
}
