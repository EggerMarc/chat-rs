use std::env;

use serde_json::Value;
use serde_json::json;
use tools_rs::ToolCollection;

use crate::core::lib::ChatOptions;
use crate::core::{
    lib::{ChatError, ChatProvider},
    messages::{Messages, content::Content},
};

const GEMINI_ENDPOINT: &str = "https://generativelanguage.googleapis.com/v1beta/models/{model_str}:generateContent?key={api_key}";

pub struct GeminiClient {
    model_name: String,
    api_key: String,
}

impl GeminiClient {
    pub fn new(model_name: &str) -> Self {
        let api_key =
            env::var("GEMINI_API_KEY").expect("Couldn't find GEMINI_API_KEY in your .env");

        GeminiClient {
            model_name: model_name.to_string(),
            api_key,
        }
    }
}

impl ChatProvider for GeminiClient {
    async fn complete(
        &self,
        messages: &Messages,
        tools: Option<&ToolCollection>,
        _options: Option<ChatOptions>,
    ) -> Result<Content, ChatError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model_name, self.api_key
        );

        let body = match tools {
            Some(t) => json!({
                "contents": messages.0,
                "tools": {
                    "functionDeclarations": t.json().unwrap()
                }
            }),
            None => json!({
                "contents": messages.0
            }),
        };

        let res = reqwest::Client::new()
            .post(url)
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| ChatError::Provider(e.to_string()))?;

        let text = res
            .text()
            .await
            .map_err(|e| ChatError::Provider(e.to_string()))?;

        let json: Value =
            serde_json::from_str(&text).map_err(|e| ChatError::InvalidResponse(e.to_string()))?;

        let content: Content = serde_json::from_value(json["candidates"][0]["content"].clone())
            .map_err(|e| ChatError::InvalidResponse(e.to_string()))?;

        Ok(content)
    }
}
