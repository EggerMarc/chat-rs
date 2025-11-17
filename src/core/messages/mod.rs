pub mod content;
pub mod parts;
pub mod text;

use content::{Content, RoleEnum};
use parts::{PartEnum, Parts};

pub fn from_user(prompts: Vec<&str>) -> Content {
    let role = RoleEnum::User;
    let parts = Parts(
        prompts
            .iter()
            .map(|prompt| PartEnum::from_text(prompt.to_string()))
            .collect(),
    );
    Content { role, parts }
}

pub fn from_system(prompts: Vec<&str>) -> Content {
    let role = RoleEnum::System;
    let parts = Parts(
        prompts
            .iter()
            .map(|prompt| PartEnum::from_text(prompt.to_string()))
            .collect(),
    );
    Content { role, parts }
}

pub fn from_model(prompts: Vec<String>) -> Content {
    let role = RoleEnum::Model;
    let parts = Parts(
        prompts
            .iter()
            .map(|prompt| PartEnum::from_text(prompt.to_string()))
            .collect(),
    );
    Content { role, parts }
}

#[derive(Clone, Debug, Default)]
#[repr(transparent)]
pub struct Messages(pub Vec<Content>);

impl Messages {
    pub fn push(&mut self, content: Content) -> &mut Self {
        self.0.push(content);
        self
    }
}
