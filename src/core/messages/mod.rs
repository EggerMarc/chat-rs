pub mod content;
pub mod parts;
pub mod structured;
pub mod text;

use content::{CompleteReasonEnum, Content, RoleEnum};
use parts::{PartEnum, Parts};

pub fn from_user(prompts: Vec<&str>) -> Messages {
    let role = RoleEnum::User;
    let parts = Parts(
        prompts
            .iter()
            .map(|prompt| PartEnum::from_text(prompt.to_string()))
            .collect(),
    );
    Messages(vec![Content {
        role,
        parts,
        complete_reason: CompleteReasonEnum::None,
    }])
}

pub fn from_system(prompts: Vec<&str>) -> Messages {
    let role = RoleEnum::System;
    let parts = Parts(
        prompts
            .iter()
            .map(|prompt| PartEnum::from_text(prompt.to_string()))
            .collect(),
    );
    Messages(vec![Content {
        role,
        parts,
        complete_reason: CompleteReasonEnum::None,
    }])
}

// TODO CompleteReasonEnum
pub fn from_model(prompts: Vec<String>) -> Messages {
    let role = RoleEnum::Model;
    let parts = Parts(
        prompts
            .iter()
            .map(|prompt| PartEnum::from_text(prompt.to_string()))
            .collect(),
    );

    Messages(vec![Content {
        role,
        parts,
        complete_reason: CompleteReasonEnum::Stop,
    }])
}

#[derive(Clone, Debug, Default)]
#[repr(transparent)]
pub struct Messages(pub Vec<Content>);

impl Messages {
    pub fn push(&mut self, content: Content) -> &mut Self {
        // We push only if content diffs from last
        if let Some(last_content) = self.0.last_mut() {
            if last_content.role == content.role {
                last_content.parts.extend(content.parts.clone());
            }
        } else {
            self.0.push(content);
        }
        self
    }

    pub fn extend(&mut self, messages: Messages) -> &mut Self {
        self.0.extend(messages.0);
        self
    }
}
