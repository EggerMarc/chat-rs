pub mod content;
pub mod parts;
pub mod structured;
pub mod text;

use content::Content;

pub fn from_user(prompts: Vec<&str>) -> Messages {
    Messages(vec![content::from_user(prompts)])
}

pub fn from_system(prompts: Vec<&str>) -> Messages {
    Messages(vec![content::from_system(prompts)])
}

// TODO CompleteReasonEnum
pub fn from_model(prompts: Vec<&str>) -> Messages {
    Messages(vec![content::from_model(prompts)])
}

#[derive(Clone, Debug, Default)]
#[repr(transparent)]
pub struct Messages(pub Vec<Content>);

impl Messages {
    pub fn push(&mut self, content: Content) -> &mut Self {
        // We push only if content diffs from last
        if let Some(last_content) = self.0.last_mut()
            && last_content.role == content.role
        {
            last_content.parts.extend(content.parts.clone());
        } else {
            self.0.push(content);
        }
        self
    }

    pub fn extend(&mut self, messages: Messages) -> &mut Self {
        self.0.extend(messages.0);
        self
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn last(&self) -> Option<&Content> {
        self.0.last()
    }
}

#[cfg(test)]
mod tests {
    use crate::messages::{
        content::{CompleteReasonEnum, RoleEnum},
        parts::{PartEnum, Parts},
    };

    use super::*;

    #[test]
    fn test_messages_push() {
        let mut messages = Messages::default();
        let sys_content = content::from_system(vec!["You are a helpful machine"]);
        let user_content = content::from_user(vec!["Hi there, I'm a user!"]);

        messages.push(sys_content).push(user_content);

        assert_eq!(messages.len(), 2);
        let thinking_part = PartEnum::from_reasoning("Thinking...".to_string());
        let model_content = Content {
            parts: Parts(vec![thinking_part]),
            role: RoleEnum::Model,
            complete_reason: CompleteReasonEnum::None,
        };

        messages.push(model_content.clone());

        assert_eq!(messages.len(), 3);
        assert_eq!(
            messages.last().expect("Last message content not found"),
            &model_content
        );

        let response_part = PartEnum::from_text("Hello there, user".to_string());
        let model_content = Content {
            parts: Parts(vec![response_part]),
            role: RoleEnum::Model,
            complete_reason: CompleteReasonEnum::Stop,
        };

        messages.push(model_content);
        assert_eq!(messages.len(), 3);
    }
}

    #[test]
    fn test_messages_push_different_roles() {
        let mut messages = Messages::default();
        let sys_content = content::from_system(vec!["System message"]);
        let user_content = content::from_user(vec!["User message"]);
        let model_content = content::from_model(vec!["Model response"]);

        messages.push(sys_content.clone());
        messages.push(user_content.clone());
        messages.push(model_content.clone());

        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn test_messages_extend() {
        let mut messages1 = Messages::default();
        messages1.push(content::from_user(vec!["First"]));

        let mut messages2 = Messages::default();
        messages2.push(content::from_system(vec!["Second"]));
        messages2.push(content::from_model(vec!["Third"]));

        messages1.extend(messages2);
        assert_eq!(messages1.len(), 3);
    }

    #[test]
    fn test_messages_extend_empty() {
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["Content"]));

        let empty_messages = Messages::default();
        messages.extend(empty_messages);

        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_messages_default() {
        let messages = Messages::default();
        assert_eq!(messages.len(), 0);
        assert!(messages.last().is_none());
    }

    #[test]
    fn test_messages_last() {
        let mut messages = Messages::default();
        assert!(messages.last().is_none());

        let content1 = content::from_user(vec!["First"]);
        let content2 = content::from_system(vec!["Second"]);

        messages.push(content1);
        messages.push(content2.clone());

        assert_eq!(messages.last(), Some(&content2));
    }

    #[test]
    fn test_messages_clone() {
        let mut messages1 = Messages::default();
        messages1.push(content::from_user(vec!["Test"]));

        let messages2 = messages1.clone();
        assert_eq!(messages1.len(), messages2.len());
    }

    #[test]
    fn test_from_user_function() {
        let messages = from_user(vec!["Hello", "World"]);
        assert_eq!(messages.len(), 1);
        
        let content = messages.last().unwrap();
        assert_eq!(content.role, RoleEnum::User);
        assert_eq!(content.parts.length(), 2);
    }

    #[test]
    fn test_from_system_function() {
        let messages = from_system(vec!["You are helpful"]);
        assert_eq!(messages.len(), 1);
        
        let content = messages.last().unwrap();
        assert_eq!(content.role, RoleEnum::System);
        assert_eq!(content.parts.length(), 1);
    }

    #[test]
    fn test_from_model_function() {
        let messages = from_model(vec!["Response"]);
        assert_eq!(messages.len(), 1);
        
        let content = messages.last().unwrap();
        assert_eq!(content.role, RoleEnum::System); // from_model uses System role
        assert_eq!(content.complete_reason, CompleteReasonEnum::Stop);
    }

    #[test]
    fn test_messages_push_merges_same_role() {
        let mut messages = Messages::default();
        let user1 = content::from_user(vec!["Part 1"]);
        let user2 = content::from_user(vec!["Part 2"]);

        messages.push(user1);
        messages.push(user2);

        // Should merge into single message with 2 parts
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.last().unwrap().parts.length(), 2);
    }

    #[test]
    fn test_messages_push_no_merge_different_roles() {
        let mut messages = Messages::default();
        let user = content::from_user(vec!["User message"]);
        let system = content::from_system(vec!["System message"]);

        messages.push(user);
        messages.push(system);

        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_messages_multiple_extends() {
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["First"]));

        let mut batch1 = Messages::default();
        batch1.push(content::from_system(vec!["Second"]));

        let mut batch2 = Messages::default();
        batch2.push(content::from_model(vec!["Third"]));

        messages.extend(batch1).extend(batch2);
        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn test_messages_with_function_calls() {
        let mut messages = Messages::default();
        let fc = tools_rs::FunctionCall::new(
            "test_function".to_string(),
            serde_json::json!({"param": "value"})
        );
        
        let mut content = Content {
            role: RoleEnum::Model,
            parts: Parts(vec![PartEnum::from_function_call(fc)]),
            complete_reason: CompleteReasonEnum::ToolCall,
        };

        messages.push(content);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.last().unwrap().parts.function_calls().len(), 1);
    }

    #[test]
    fn test_messages_push_empty_parts() {
        let mut messages = Messages::default();
        let content = Content {
            role: RoleEnum::User,
            parts: Parts::default(),
            complete_reason: CompleteReasonEnum::None,
        };

        messages.push(content);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.last().unwrap().parts.length(), 0);
    }

    #[test]
    fn test_messages_conversation_flow() {
        let mut messages = Messages::default();
        
        // System prompt
        messages.push(content::from_system(vec!["Be helpful"]));
        assert_eq!(messages.len(), 1);
        
        // User message
        messages.push(content::from_user(vec!["Hello"]));
        assert_eq!(messages.len(), 2);
        
        // Model response with reasoning
        let reasoning_part = PartEnum::from_reasoning("Analyzing request".to_string());
        let text_part = PartEnum::from_text("Hi there!".to_string());
        let mut model_content = Content {
            role: RoleEnum::Model,
            parts: Parts(vec![reasoning_part, text_part]),
            complete_reason: CompleteReasonEnum::Stop,
        };
        messages.push(model_content);
        assert_eq!(messages.len(), 3);
        
        // Another user message
        messages.push(content::from_user(vec!["Thanks"]));
        assert_eq!(messages.len(), 4);
    }

    #[test]
    fn test_from_user_empty_vec() {
        let messages = from_user(vec![]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.last().unwrap().parts.length(), 0);
    }

    #[test]
    fn test_from_system_empty_vec() {
        let messages = from_system(vec![]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.last().unwrap().parts.length(), 0);
    }

    #[test]
    fn test_from_model_empty_vec() {
        let messages = from_model(vec![]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.last().unwrap().parts.length(), 0);
    }

    #[test]
    fn test_messages_with_special_characters() {
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["Hello\nWorld", "Tab\there", "Quote\"test"]));
        
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.last().unwrap().parts.length(), 3);
    }

    #[test]
    fn test_messages_with_unicode() {
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["Hello 世界", "Emoji 🚀"]));
        
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.last().unwrap().parts.length(), 2);
    }
