use reqwest::Client;

pub struct ClaudeClient {
    pub(crate) model_name: String,
    pub(crate) api_key: String,
    pub(crate) api_version: String,
    pub(crate) http_client: Client,
}
