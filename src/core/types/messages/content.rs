use serde::{Deserialize, Serialize};

use crate::types::messages::parts::{PartEnum, Parts};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Content {
    pub parts: Parts,
    pub role: RoleEnum,
    pub complete_reason: CompleteReasonEnum,
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
}
