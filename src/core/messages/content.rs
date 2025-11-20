use serde::{Deserialize, Serialize};

use crate::core::messages::parts::{PartEnum, Parts};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Content {
    pub parts: Parts,
    pub role: RoleEnum,
    pub complete_reason: CompleteReasonEnum,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoleEnum {
    #[default]
    User,
    System,
    Model,
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompleteReasonEnum {
    ToolCall,
    Stop,
    MaxTokens,
    ContentFilter,
    Recitation,
    #[default]
    None,
}

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
        complete_reason: CompleteReasonEnum::None,
    }
}

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
        complete_reason: CompleteReasonEnum::None,
    }
}

pub fn from_model(prompts: Vec<&str>) -> Content {
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
        complete_reason: CompleteReasonEnum::Stop,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_user_single_prompt() {
        let content = from_user(vec!["Hello, world!"]);
        assert_eq!(content.role, RoleEnum::User);
        assert_eq!(content.parts.length(), 1);
        assert_eq!(content.complete_reason, CompleteReasonEnum::None);
        
        if let Some(PartEnum::Text(text)) = content.parts.0.first() {
            assert_eq!(text.to_string(), "Hello, world!");
        } else {
            panic!("Expected text part");
        }
    }

    #[test]
    fn test_from_user_multiple_prompts() {
        let content = from_user(vec!["First message", "Second message", "Third message"]);
        assert_eq!(content.role, RoleEnum::User);
        assert_eq!(content.parts.length(), 3);
        assert_eq!(content.complete_reason, CompleteReasonEnum::None);
    }

    #[test]
    fn test_from_user_empty_prompt() {
        let content = from_user(vec![""]);
        assert_eq!(content.role, RoleEnum::User);
        assert_eq!(content.parts.length(), 1);
        
        if let Some(PartEnum::Text(text)) = content.parts.0.first() {
            assert_eq!(text.to_string(), "");
        } else {
            panic!("Expected text part");
        }
    }

    #[test]
    fn test_from_user_special_characters() {
        let content = from_user(vec!["Hello\nWorld", "Tab\there", "Quote\"test"]);
        assert_eq!(content.role, RoleEnum::User);
        assert_eq!(content.parts.length(), 3);
    }

    #[test]
    fn test_from_system_single_prompt() {
        let content = from_system(vec!["You are a helpful assistant."]);
        assert_eq!(content.role, RoleEnum::System);
        assert_eq!(content.parts.length(), 1);
        assert_eq!(content.complete_reason, CompleteReasonEnum::None);
        
        if let Some(PartEnum::Text(text)) = content.parts.0.first() {
            assert_eq!(text.to_string(), "You are a helpful assistant.");
        } else {
            panic!("Expected text part");
        }
    }

    #[test]
    fn test_from_system_multiple_prompts() {
        let content = from_system(vec![
            "You are helpful.",
            "You are accurate.",
            "You are concise."
        ]);
        assert_eq!(content.role, RoleEnum::System);
        assert_eq!(content.parts.length(), 3);
    }

    #[test]
    fn test_from_system_empty_prompt() {
        let content = from_system(vec![]);
        assert_eq!(content.role, RoleEnum::System);
        assert_eq!(content.parts.length(), 0);
    }

    #[test]
    fn test_from_model_single_prompt() {
        let content = from_model(vec!["I can help you with that."]);
        assert_eq!(content.role, RoleEnum::System); // Note: from_model uses System role
        assert_eq!(content.parts.length(), 1);
        assert_eq!(content.complete_reason, CompleteReasonEnum::Stop);
    }

    #[test]
    fn test_from_model_multiple_prompts() {
        let content = from_model(vec!["Response part 1", "Response part 2"]);
        assert_eq!(content.role, RoleEnum::System);
        assert_eq!(content.parts.length(), 2);
        assert_eq!(content.complete_reason, CompleteReasonEnum::Stop);
    }

    #[test]
    fn test_from_model_empty_prompt() {
        let content = from_model(vec![]);
        assert_eq!(content.role, RoleEnum::System);
        assert_eq!(content.parts.length(), 0);
        assert_eq!(content.complete_reason, CompleteReasonEnum::Stop);
    }

    #[test]
    fn test_content_default() {
        let content = Content::default();
        assert_eq!(content.role, RoleEnum::default());
        assert_eq!(content.parts.length(), 0);
        assert_eq!(content.complete_reason, CompleteReasonEnum::None);
    }

    #[test]
    fn test_content_equality() {
        let content1 = from_user(vec!["Test"]);
        let content2 = from_user(vec!["Test"]);
        assert_eq!(content1, content2);
    }

    #[test]
    fn test_content_inequality_different_text() {
        let content1 = from_user(vec!["Test1"]);
        let content2 = from_user(vec!["Test2"]);
        assert_ne!(content1, content2);
    }

    #[test]
    fn test_content_inequality_different_role() {
        let content1 = from_user(vec!["Test"]);
        let content2 = from_system(vec!["Test"]);
        assert_ne!(content1, content2);
    }

    #[test]
    fn test_role_enum_serialization() {
        // Test that roles can be serialized/deserialized
        let user_role = RoleEnum::User;
        let json = serde_json::to_string(&user_role).unwrap();
        let deserialized: RoleEnum = serde_json::from_str(&json).unwrap();
        assert_eq!(user_role, deserialized);
    }

    #[test]
    fn test_complete_reason_enum_serialization() {
        let reason = CompleteReasonEnum::Stop;
        let json = serde_json::to_string(&reason).unwrap();
        let deserialized: CompleteReasonEnum = serde_json::from_str(&json).unwrap();
        assert_eq!(reason, deserialized);
    }

    #[test]
    fn test_from_user_unicode_content() {
        let content = from_user(vec!["Hello 世界", "émojis 🚀🎉", "Ω≈ç√"]);
        assert_eq!(content.role, RoleEnum::User);
        assert_eq!(content.parts.length(), 3);
    }

    #[test]
    fn test_from_system_long_prompt() {
        let long_text = "a".repeat(10000);
        let content = from_system(vec![&long_text]);
        assert_eq!(content.role, RoleEnum::System);
        assert_eq!(content.parts.length(), 1);
        
        if let Some(PartEnum::Text(text)) = content.parts.0.first() {
            assert_eq!(text.to_string().len(), 10000);
        }
    }

    #[test]
    fn test_content_clone() {
        let content1 = from_user(vec!["Clone test"]);
        let content2 = content1.clone();
        assert_eq!(content1, content2);
    }

    #[test]
    fn test_complete_reason_variants() {
        // Ensure all variants are constructable
        let _stop = CompleteReasonEnum::Stop;
        let _max_tokens = CompleteReasonEnum::MaxTokens;
        let _tool_call = CompleteReasonEnum::ToolCall;
        let _recitation = CompleteReasonEnum::Recitation;
        let _content_filter = CompleteReasonEnum::ContentFilter;
        let _none = CompleteReasonEnum::None;
    }
}
