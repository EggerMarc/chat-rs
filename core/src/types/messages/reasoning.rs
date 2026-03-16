use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Reasoning {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
}

impl Reasoning {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            signature: None,
        }
    }

    pub fn with_signature(mut self, signature: String) -> Self {
        self.signature = Some(signature);
        self
    }
}

impl Display for Reasoning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}

impl From<String> for Reasoning {
    fn from(value: String) -> Reasoning {
        Reasoning::new(value)
    }
}

impl From<Reasoning> for String {
    fn from(value: Reasoning) -> String {
        value.text
    }
}
