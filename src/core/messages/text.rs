use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Text(pub String);

impl Text {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Into<String> for Text {
    fn into(self) -> String {
        self.0
    }
}

impl From<String> for Text {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Display for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_new() {
        let text = Text::new("Hello, world!");
        assert_eq!(text.to_string(), "Hello, world!");
    }

    #[test]
    fn test_text_new_empty() {
        let text = Text::new("");
        assert_eq!(text.to_string(), "");
    }

    #[test]
    fn test_text_from_string() {
        let s = String::from("Test string");
        let text = Text::from(s);
        assert_eq!(text.to_string(), "Test string");
    }

    #[test]
    fn test_text_from_str() {
        let text = Text::from("Static str");
        assert_eq!(text.to_string(), "Static str");
    }

    #[test]
    fn test_text_default() {
        let text = Text::default();
        assert_eq!(text.to_string(), "");
    }

    #[test]
    fn test_text_clone() {
        let text1 = Text::new("Clone me");
        let text2 = text1.clone();
        assert_eq!(text1.to_string(), text2.to_string());
    }

    #[test]
    fn test_text_debug() {
        let text = Text::new("Debug test");
        let debug_string = format!("{:?}", text);
        assert!(debug_string.contains("Text"));
    }

    #[test]
    fn test_text_display() {
        let text = Text::new("Display me");
        let display_string = format!("{}", text);
        assert_eq!(display_string, "Display me");
    }

    #[test]
    fn test_text_equality() {
        let text1 = Text::new("Same");
        let text2 = Text::new("Same");
        assert_eq!(text1, text2);
    }

    #[test]
    fn test_text_inequality() {
        let text1 = Text::new("Different1");
        let text2 = Text::new("Different2");
        assert_ne!(text1, text2);
    }

    #[test]
    fn test_text_serialization() {
        let text = Text::new("Serialize me");
        let json = serde_json::to_string(&text).unwrap();
        let deserialized: Text = serde_json::from_str(&json).unwrap();
        assert_eq!(text, deserialized);
    }

    #[test]
    fn test_text_with_newlines() {
        let text = Text::new("Line1\nLine2\nLine3");
        assert!(text.to_string().contains('\n'));
    }

    #[test]
    fn test_text_with_tabs() {
        let text = Text::new("Col1\tCol2\tCol3");
        assert!(text.to_string().contains('\t'));
    }

    #[test]
    fn test_text_with_quotes() {
        let text = Text::new("He said \"Hello\"");
        assert!(text.to_string().contains('"'));
    }

    #[test]
    fn test_text_unicode() {
        let text = Text::new("Hello 世界 🌍");
        assert_eq!(text.to_string(), "Hello 世界 🌍");
    }

    #[test]
    fn test_text_long_string() {
        let long_string = "a".repeat(10000);
        let text = Text::new(&long_string);
        assert_eq!(text.to_string().len(), 10000);
    }

    #[test]
    fn test_text_special_characters() {
        let text = Text::new("!@#$%^&*()_+-={}[]|\\:;\"'<>?,./");
        assert!(text.to_string().contains('!'));
        assert!(text.to_string().contains('$'));
    }

    #[test]
    fn test_text_whitespace_only() {
        let text = Text::new("   \t\n  ");
        assert!(!text.to_string().is_empty());
    }

    #[test]
    fn test_text_from_owned_string() {
        let owned = String::from("Owned string");
        let text = Text::from(owned.clone());
        assert_eq!(text.to_string(), owned);
    }

    #[test]
    fn test_text_multiple_from_conversions() {
        let text1 = Text::from("str");
        let text2 = Text::from(String::from("String"));
        let text3 = Text::new("new");
        
        assert_eq!(text1.to_string(), "str");
        assert_eq!(text2.to_string(), "String");
        assert_eq!(text3.to_string(), "new");
    }

    #[test]
    fn test_text_empty_variants() {
        let text1 = Text::new("");
        let text2 = Text::from("");
        let text3 = Text::from(String::new());
        let text4 = Text::default();
        
        assert_eq!(text1, text2);
        assert_eq!(text2, text3);
        assert_eq!(text3, text4);
    }
}
