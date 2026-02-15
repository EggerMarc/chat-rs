use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;
use tools_rs::ToolCollection;

use crate::{
    core::messages::{Messages, content::Content},
    metadata::Metadata,
};
use async_trait::async_trait;

#[derive(Clone)]
pub struct ChatResponse {
    pub metadata: Option<Metadata>,
    pub content: Content,
}

pub struct ChatFailure {
    pub metadata: Option<Metadata>,
    pub err: ChatError,
}

#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn complete(
        &self,
        messages: &Messages,
        tools: Option<&ToolCollection>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatError>;
}

/*
#[async_trait]
pub trait ChatStreamProvider {
    async fn stream() -> {

    }
}
*/

#[derive(Debug, Clone, Default)]
pub struct ChatOptions {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub metadata: HashMap<String, Value>, // provider-specific extensions
}

#[derive(Clone, Debug, Error)]
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
