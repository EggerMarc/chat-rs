use std::fmt::Display;

use crate::messages::metadata::Metadata;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Text {
    pub text: String,
    pub meta: Option<Metadata>,
}

impl Text {
    pub fn new(s: impl Into<String>) -> Self {
        Self {
            text: s.into(),
            meta: None,
        }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

impl Into<String> for Text {
    fn into(self) -> String {
        self.text
    }
}

impl From<String> for Text {
    fn from(s: String) -> Self {
        Self {
            text: s,
            meta: None,
        }
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Self {
            text: s.to_string(),
            meta: None,
        }
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        &self.text
    }
}

impl Display for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.text)
    }
}
