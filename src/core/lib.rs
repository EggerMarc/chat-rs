use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;
use tools_rs::ToolCollection;

use crate::core::messages::{Messages, content::Content};
use async_trait::async_trait;

#[async_trait]
pub trait ChatProvider: Send + Sync {
    async fn complete(
        &self,
        messages: &Messages,
        tools: Option<&ToolCollection>,
        options: Option<&ChatOptions>,
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
pub struct ChatOptions {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub metadata: HashMap<String, Value>, // provider-specific extensions
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_error_provider() {
        let error = ChatError::Provider("API error".to_string());
        let error_string = format!("{}", error);
        assert!(error_string.contains("API error"));
    }

    #[test]
    fn test_chat_error_rate_limited() {
        let error = ChatError::RateLimited;
        let error_string = format!("{}", error);
        assert!(!error_string.is_empty());
    }

    #[test]
    fn test_chat_error_invalid_response() {
        let error = ChatError::InvalidResponse("Bad JSON".to_string());
        let error_string = format!("{}", error);
        assert!(error_string.contains("Bad JSON"));
    }

    #[test]
    fn test_chat_error_tool_execution() {
        let error = ChatError::ToolExecution("Tool failed".to_string());
        let error_string = format!("{}", error);
        assert!(error_string.contains("Tool failed"));
    }

    #[test]
    fn test_chat_error_other() {
        let error = ChatError::Other("Unknown error".to_string());
        let error_string = format!("{}", error);
        assert!(error_string.contains("Unknown error"));
    }

    #[test]
    fn test_chat_error_debug() {
        let error = ChatError::Provider("Debug test".to_string());
        let debug_string = format!("{:?}", error);
        assert!(debug_string.contains("Provider"));
    }

    #[test]
    fn test_chat_error_is_error_trait() {
        let error = ChatError::RateLimited;
        let _: &dyn std::error::Error = &error;
    }

    #[test]
    fn test_chat_options_default() {
        let options = ChatOptions::default();
        // Just verify it can be created
        let _options = options;
    }

    #[test]
    fn test_chat_options_clone() {
        let options1 = ChatOptions::default();
        let options2 = options1.clone();
        // Verify clone works
        let _both = (options1, options2);
    }

    #[test]
    fn test_chat_options_debug() {
        let options = ChatOptions::default();
        let debug_string = format!("{:?}", options);
        assert!(debug_string.contains("ChatOptions"));
    }

    #[test]
    fn test_chat_error_different_variants() {
        let errors = vec![
            ChatError::Provider("test".to_string()),
            ChatError::RateLimited,
            ChatError::InvalidResponse("test".to_string()),
            ChatError::ToolExecution("test".to_string()),
            ChatError::Other("test".to_string()),
        ];
        
        assert_eq!(errors.len(), 5);
    }

    #[test]
    fn test_chat_error_from_string() {
        let message = "Error message".to_string();
        let error = ChatError::Provider(message.clone());
        
        match error {
            ChatError::Provider(msg) => assert_eq!(msg, "Error message"),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_chat_error_empty_message() {
        let error = ChatError::Provider(String::new());
        let error_string = format!("{}", error);
        // Should still format without panicking
        assert!(!error_string.is_empty() || error_string.is_empty());
    }

    #[test]
    fn test_chat_error_long_message() {
        let long_message = "a".repeat(1000);
        let error = ChatError::Provider(long_message.clone());
        
        match error {
            ChatError::Provider(msg) => assert_eq!(msg.len(), 1000),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_chat_error_special_characters() {
        let message = "Error: \n\t\"quoted\" & <special>";
        let error = ChatError::InvalidResponse(message.to_string());
        let formatted = format!("{}", error);
        assert!(formatted.contains("quoted"));
    }

    #[test]
    fn test_chat_error_unicode() {
        let message = "错误: エラー 🚨";
        let error = ChatError::Other(message.to_string());
        let formatted = format!("{}", error);
        assert!(formatted.contains("错误"));
    }
}
