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
        complete_reason: CompleteReasonEnum::None,
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
        complete_reason: CompleteReasonEnum::None,
    }
}

/// Constructs a Content with the System role from model-generated prompt strings.
///
/// Each prompt is converted into a Part and collected into `parts`. The `complete_reason` is set to `CompleteReasonEnum::Stop`.
///
/// # Examples
///
/// ```
/// let content = from_model(vec!["generated text"]);
/// assert_eq!(content.role, RoleEnum::System);
/// assert!(matches!(content.complete_reason, CompleteReasonEnum::Stop));
/// assert_eq!(content.parts.0.len(), 1);
/// ```
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