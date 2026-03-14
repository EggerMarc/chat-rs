use crate::tools::OpenAINativeTool;

pub struct OpenAIClient {
    pub(crate) model_name: Option<String>,
    pub(crate) api_key: Option<String>,
    pub(crate) base_url: String,
    pub(crate) native_tools: Vec<Box<dyn OpenAINativeTool>>,
    pub(crate) http_client: reqwest::Client,
}
