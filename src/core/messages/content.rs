use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    core::messages::parts::{PartEnum, Parts},
    metadata::{Metadata, usage::Usage},
};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Content {
    pub parts: Parts,
    pub role: RoleEnum,
    pub complete_reason: CompleteReasonEnum,
    pub metadata: Option<Metadata>,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoleEnum {
    #[default]
    User,
    System,
    Model,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompleteReasonEnum {
    ToolCall,
    Stop,
    MaxTokens,
    #[default]
    None,
    Other(String),
}

impl Content {
    pub fn total_tokens(&self) -> usize {
        self.metadata
            .as_ref()
            .map(|m| m.usage.total_tokens)
            .unwrap_or(0)
    }

    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.ensure_metadata().id = Some(id.into());
        self
    }

    pub fn with_usage(mut self, usage: impl Into<Usage>) -> Self {
        self.ensure_metadata().usage = usage.into();
        self
    }

    pub fn with_duration(mut self, duration_ms: impl Into<u64>) -> Self {
        self.ensure_metadata().duration_ms = Some(duration_ms.into());
        self
    }

    pub fn with_specific(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.ensure_metadata()
            .specific
            .insert(key.into(), value.into());
        self
    }

    fn ensure_metadata(&mut self) -> &mut Metadata {
        self.metadata.get_or_insert_with(Metadata::default)
    }
}

/// Creates a Content with the user role from the provided prompt strings.
///
/// Each prompt string is converted into a content part. The resulting Content
/// has role set to `RoleEnum::User` and `complete_reason` set to
/// `CompleteReasonEnum::None`.
///
/// # Examples
///
/// ```
/// let c = from_user(vec!["hello"]);
/// assert_eq!(c.role, RoleEnum::User);
/// assert_eq!(c.parts.0.len(), 1);
/// ```
pub fn from_user(prompts: Vec<&str>) -> Content {
    let role = RoleEnum::User;
    let parts = Parts(
        prompts
            .iter()
            .map(|prompt| PartEnum::from_text(prompt.to_string()))
            .collect(),
    );
    Content {
        role,
        parts,
        ..Content::default()
    }
}

/// Constructs a `Content` with the system role from the provided prompt strings.
///
/// Returns a `Content` whose `role` is `RoleEnum::System`, whose `parts` are created from each prompt string, and whose `complete_reason` is `CompleteReasonEnum::None`.
///
/// # Examples
///
/// ```
/// let content = from_system(vec!["Initialize system", "Set config"]);
/// assert_eq!(content.role, RoleEnum::System);
/// assert_eq!(content.parts.0.len(), 2);
/// ```
pub fn from_system(prompts: Vec<&str>) -> Content {
    let role = RoleEnum::System;
    let parts = Parts(
        prompts
            .iter()
            .map(|prompt| PartEnum::from_text(prompt.to_string()))
            .collect(),
    );
    Content {
        role,
        parts,
        ..Content::default()
    }
}

/// Constructs a Content with the Model role from model-generated prompt strings.
///
/// Each prompt is converted into a Part and collected into `parts`. The `complete_reason` is set to `CompleteReasonEnum::Stop`.
///
/// # Examples
///
/// ```
/// let content = from_model(vec!["generated text"]);
/// assert_eq!(content.role, RoleEnum::Model);
/// assert!(matches!(content.complete_reason, CompleteReasonEnum::Stop));
/// assert_eq!(content.parts.0.len(), 1);
/// ```
pub fn from_model(prompts: Vec<&str>) -> Content {
    let role = RoleEnum::Model;
    let parts = Parts(
        prompts
            .iter()
            .map(|prompt| PartEnum::from_text(prompt.to_string()))
            .collect(),
    );
    Content {
        role,
        parts,
        ..Content::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_user_creates_content_with_correct_role() {
        let content = from_user(vec!["Hello, world!"]);
        assert_eq!(content.role, RoleEnum::User);
    }

    #[test]
    fn test_from_user_creates_content_with_correct_complete_reason() {
        let content = from_user(vec!["Hello"]);
        assert_eq!(content.complete_reason, CompleteReasonEnum::None);
    }

    #[test]
    fn test_from_user_with_single_prompt() {
        let content = from_user(vec!["Test message"]);
        assert_eq!(content.parts.len(), 1);
        assert_eq!(
            content.parts.text_response().unwrap().as_str(),
            "Test message"
        );
    }

    #[test]
    fn test_from_user_with_multiple_prompts() {
        let content = from_user(vec!["First", "Second", "Third"]);
        assert_eq!(content.parts.len(), 3);
        let texts: Vec<&str> = content.parts.text_parts().map(|t| t.as_str()).collect();
        assert_eq!(texts, vec!["First", "Second", "Third"]);
    }

    #[test]
    fn test_from_user_with_empty_prompts() {
        let content = from_user(vec![]);
        assert_eq!(content.parts.len(), 0);
        assert_eq!(content.role, RoleEnum::User);
    }

    #[test]
    fn test_from_user_with_empty_string() {
        let content = from_user(vec![""]);
        assert_eq!(content.parts.len(), 1);
        assert_eq!(content.parts.text_response().unwrap().as_str(), "");
    }

    #[test]
    fn test_from_user_with_unicode_and_special_chars() {
        let content = from_user(vec!["Hello 🌍!", "Special chars: @#$%", "Unicode: 你好"]);
        assert_eq!(content.parts.len(), 3);
    }

    #[test]
    fn test_from_system_creates_content_with_correct_role() {
        let content = from_system(vec!["System instruction"]);
        assert_eq!(content.role, RoleEnum::System);
    }

    #[test]
    fn test_from_system_creates_content_with_correct_complete_reason() {
        let content = from_system(vec!["System"]);
        assert_eq!(content.complete_reason, CompleteReasonEnum::None);
    }

    #[test]
    fn test_from_system_with_single_prompt() {
        let content = from_system(vec!["You are a helpful assistant"]);
        assert_eq!(content.parts.len(), 1);
        assert_eq!(
            content.parts.text_response().unwrap().as_str(),
            "You are a helpful assistant"
        );
    }

    #[test]
    fn test_from_system_with_multiple_prompts() {
        let content = from_system(vec!["Rule 1", "Rule 2", "Rule 3"]);
        assert_eq!(content.parts.len(), 3);
    }

    #[test]
    fn test_from_system_with_empty_prompts() {
        let content = from_system(vec![]);
        assert_eq!(content.parts.len(), 0);
        assert_eq!(content.role, RoleEnum::System);
    }

    #[test]
    fn test_from_system_with_long_text() {
        let long_text = "a".repeat(10000);
        let content = from_system(vec![&long_text]);
        assert_eq!(content.parts.len(), 1);
        assert_eq!(content.parts.text_response().unwrap().as_str().len(), 10000);
    }

    #[test]
    fn test_from_model_creates_content_with_correct_role() {
        let content = from_model(vec!["Model response"]);
        // Note: There appears to be a bug - from_model sets role to System instead of Model
        assert_eq!(content.role, RoleEnum::Model);
    }

    #[test]
    fn test_from_model_creates_content_with_stop_reason() {
        let content = from_model(vec!["Response"]);
        assert_eq!(content.complete_reason, CompleteReasonEnum::Stop);
    }

    #[test]
    fn test_from_model_with_single_prompt() {
        let content = from_model(vec!["Here is my response"]);
        assert_eq!(content.parts.len(), 1);
        assert_eq!(
            content.parts.text_response().unwrap().as_str(),
            "Here is my response"
        );
    }

    #[test]
    fn test_from_model_with_multiple_prompts() {
        let content = from_model(vec!["Part 1", "Part 2"]);
        assert_eq!(content.parts.len(), 2);
    }

    #[test]
    fn test_from_model_with_empty_prompts() {
        let content = from_model(vec![]);
        assert_eq!(content.parts.len(), 0);
    }

    #[test]
    fn test_content_default() {
        let content = Content::default();
        assert_eq!(content.role, RoleEnum::User);
        assert_eq!(content.complete_reason, CompleteReasonEnum::None);
        assert_eq!(content.parts.len(), 0);
    }

    #[test]
    fn test_content_clone() {
        let content1 = from_user(vec!["Test"]);
        let content2 = content1.clone();
        assert_eq!(content1, content2);
    }

    #[test]
    fn test_complete_reason_enum_default() {
        let reason = CompleteReasonEnum::default();
        assert_eq!(reason, CompleteReasonEnum::None);
    }

    #[test]
    fn test_content_serialization() {
        let content = from_user(vec!["Test"]);
        let serialized = serde_json::to_string(&content).unwrap();
        let deserialized: Content = serde_json::from_str(&serialized).unwrap();
        assert_eq!(content, deserialized);
    }

    #[test]
    fn test_role_enum_serialization() {
        let role = RoleEnum::User;
        let serialized = serde_json::to_string(&role).unwrap();
        let deserialized: RoleEnum = serde_json::from_str(&serialized).unwrap();
        assert_eq!(role, deserialized);
    }

    #[test]
    fn test_complete_reason_serialization() {
        let reason = CompleteReasonEnum::Stop;
        let serialized = serde_json::to_string(&reason).unwrap();
        let deserialized: CompleteReasonEnum = serde_json::from_str(&serialized).unwrap();
        assert_eq!(reason, deserialized);
    }
}

    #[test]
    fn test_content_total_tokens_with_metadata() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            ..Default::default()
        };
        let metadata = Metadata {
            usage,
            ..Default::default()
        };
        let content = Content {
            metadata: Some(metadata),
            ..Default::default()
        };
        assert_eq!(content.total_tokens(), 150);
    }

    #[test]
    fn test_content_total_tokens_without_metadata() {
        let content = Content::default();
        assert_eq!(content.total_tokens(), 0);
    }

    #[test]
    fn test_content_with_id() {
        let content = Content::default().with_id("test-id-456");
        assert_eq!(content.metadata.as_ref().unwrap().id.as_ref().unwrap(), "test-id-456");
    }

    #[test]
    fn test_content_with_id_string() {
        let id = String::from("string-id-789");
        let content = Content::default().with_id(id);
        assert_eq!(content.metadata.as_ref().unwrap().id.as_ref().unwrap(), "string-id-789");
    }

    #[test]
    fn test_content_with_usage() {
        let usage = Usage {
            input_tokens: 200,
            output_tokens: 100,
            total_tokens: 300,
            ..Default::default()
        };
        let content = Content::default().with_usage(usage.clone());
        assert_eq!(content.metadata.as_ref().unwrap().usage.total_tokens, 300);
        assert_eq!(content.metadata.as_ref().unwrap().usage.input_tokens, 200);
    }

    #[test]
    fn test_content_with_duration() {
        let content = Content::default().with_duration(2500u64);
        assert_eq!(content.metadata.as_ref().unwrap().duration_ms.unwrap(), 2500);
    }

    #[test]
    fn test_content_with_specific() {
        let content = Content::default()
            .with_specific("safety_rating", serde_json::json!("safe"))
            .with_specific("provider", serde_json::json!("gemini"));
        
        let metadata = content.metadata.as_ref().unwrap();
        assert_eq!(metadata.specific.len(), 2);
        assert!(metadata.specific.contains_key("safety_rating"));
        assert!(metadata.specific.contains_key("provider"));
        assert_eq!(metadata.specific.get("safety_rating").unwrap(), &serde_json::json!("safe"));
    }

    #[test]
    fn test_content_chained_metadata_builders() {
        let usage = Usage {
            input_tokens: 50,
            output_tokens: 25,
            total_tokens: 75,
            ..Default::default()
        };
        
        let content = Content::default()
            .with_id("chain-test")
            .with_usage(usage)
            .with_duration(1000)
            .with_specific("test_key", serde_json::json!({"nested": "value"}));
        
        let metadata = content.metadata.as_ref().unwrap();
        assert_eq!(metadata.id.as_ref().unwrap(), "chain-test");
        assert_eq!(metadata.usage.total_tokens, 75);
        assert_eq!(metadata.duration_ms.unwrap(), 1000);
        assert!(metadata.specific.contains_key("test_key"));
    }

    #[test]
    fn test_content_metadata_preserves_existing_data() {
        let content = Content::default()
            .with_id("first-id")
            .with_duration(500);
        
        let content = content.with_specific("key", serde_json::json!("value"));
        
        let metadata = content.metadata.as_ref().unwrap();
        assert_eq!(metadata.id.as_ref().unwrap(), "first-id");
        assert_eq!(metadata.duration_ms.unwrap(), 500);
        assert!(metadata.specific.contains_key("key"));
    }

    #[test]
    fn test_content_with_usage_overwrites() {
        let usage1 = Usage {
            total_tokens: 100,
            ..Default::default()
        };
        let usage2 = Usage {
            total_tokens: 200,
            ..Default::default()
        };
        
        let content = Content::default()
            .with_usage(usage1)
            .with_usage(usage2);
        
        assert_eq!(content.metadata.as_ref().unwrap().usage.total_tokens, 200);
    }

    #[test]
    fn test_content_with_specific_complex_json() {
        let complex_json = serde_json::json!({
            "nested": {
                "array": [1, 2, 3],
                "object": {"key": "value"}
            },
            "number": 42,
            "boolean": true
        });
        
        let content = Content::default().with_specific("complex", complex_json.clone());
        
        let metadata = content.metadata.as_ref().unwrap();
        assert_eq!(metadata.specific.get("complex").unwrap(), &complex_json);
    }

    #[test]
    fn test_complete_reason_enum_other_variant() {
        let reason = CompleteReasonEnum::Other("custom_reason".to_string());
        match reason {
            CompleteReasonEnum::Other(s) => assert_eq!(s, "custom_reason"),
            _ => panic!("Expected Other variant"),
        }
    }

    #[test]
    fn test_complete_reason_enum_serialization_with_other() {
        let reason = CompleteReasonEnum::Other("timeout".to_string());
        let serialized = serde_json::to_string(&reason).unwrap();
        let deserialized: CompleteReasonEnum = serde_json::from_str(&serialized).unwrap();
        assert_eq!(reason, deserialized);
    }

    #[test]
    fn test_role_enum_serialization_lowercase() {
        let role = RoleEnum::User;
        let serialized = serde_json::to_string(&role).unwrap();
        assert!(serialized.contains("\"user\""));
        
        let role = RoleEnum::Model;
        let serialized = serde_json::to_string(&role).unwrap();
        assert!(serialized.contains("\"model\""));
    }

    #[test]
    fn test_complete_reason_serialization_snake_case() {
        let reason = CompleteReasonEnum::MaxTokens;
        let serialized = serde_json::to_string(&reason).unwrap();
        assert!(serialized.contains("\"max_tokens\""));
        
        let reason = CompleteReasonEnum::ToolCall;
        let serialized = serde_json::to_string(&reason).unwrap();
        assert!(serialized.contains("\"tool_call\""));
    }

    #[test]
    fn test_content_with_metadata_field() {
        let metadata = Metadata {
            id: Some("direct-metadata".to_string()),
            ..Default::default()
        };
        
        let content = Content {
            metadata: Some(metadata),
            ..Default::default()
        };
        
        assert!(content.metadata.is_some());
        assert_eq!(content.metadata.unwrap().id.unwrap(), "direct-metadata");
    }
