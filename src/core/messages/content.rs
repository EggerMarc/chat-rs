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
