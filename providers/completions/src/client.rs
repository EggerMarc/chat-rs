use chat_core::transport::Transport;
use chat_core::types::provider_meta::ProviderMeta;

pub struct ChatCompletionsClient<T: Transport> {
    pub(crate) model_name: String,
    pub(crate) api_key: Option<String>,
    pub(crate) scheme: String,
    pub(crate) host: String,
    pub(crate) base_path: String,
    pub(crate) extra_headers: Vec<(String, String)>,
    pub(crate) transport: T,
    pub(crate) meta: ProviderMeta,
}

impl<T: Transport> ChatCompletionsClient<T> {
    pub(crate) fn build_headers(&self) -> Vec<(String, String)> {
        let mut headers = vec![("Content-Type".into(), "application/json".into())];
        if let Some(key) = &self.api_key {
            headers.push(("Authorization".into(), format!("Bearer {key}")));
        }
        headers.extend(self.extra_headers.iter().cloned());
        headers
    }
}
