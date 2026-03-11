use reqwest::Client;

use crate::{tools::GeminiNativeTool, FunctionCallingConfig, GeminiBuilder};

pub(crate) struct FunctionCallingConfig {
    pub(crate) mode: String,
    pub(crate) config: String,
}

pub struct GeminiClient {
    pub(crate) model_name: String,
    pub(crate) api_key: String,
    pub(crate) http_client: Client,
    pub(crate) native_tools: Vec<Box<dyn GeminiNativeTool>>,
    pub(crate) function_config: Option<FunctionCallingConfig>,
}

impl GeminiBuilder {
    pub fn build(self) -> GeminiClient {
        GeminiClient {
            model_name: self.model_name.unwrap_or_else(|| "gemini-2.0-flash".into()),
            api_key: self.api_key.expect("API Key required"),
            http_client: Client::new(), // Create once!
            native_tools: self.native_tools,
            function_config: self.function_config,
        }
    }
}
