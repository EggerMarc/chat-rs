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
