use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;
use tools_rs::ToolCollection;

use crate::core::messages::{Messages, content::Content};
use async_trait::async_trait;

#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn complete<Shape: serde::de::DeserializeOwned + Default + Clone>(
        &self,
        messages: &Messages,
        tools: Option<&ToolCollection>,
        options: ChatOptions<Shape>,
    ) -> Result<Content, ChatError>;
}

/*
#[async_trait]
pub trait ChatStreamProvider {
    async fn stream() -> {

    }
}
*/

#[derive(Debug, Clone, Default)]
pub struct ChatOptions<Shape: serde::de::DeserializeOwned + Default + Clone> {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub metadata: HashMap<String, Value>, // provider-specific extensions
    pub structured_output: Option<Shape>,
}

#[derive(Debug, Error)]
pub enum ChatError {
    #[error("network error: {0}")]
    Network(String),

    #[error("provider error: {0}")]
    Provider(String),

    #[error("rate limited")]
    RateLimited,

    #[error("invalid response: {0}")]
    InvalidResponse(String),

    #[error("unknown error: {0}")]
    Other(String),
}
