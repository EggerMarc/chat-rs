pub mod content;
pub mod embeddings;
pub mod file;
pub mod parts;
pub mod text;

use content::Content;

/// Create a `Messages` containing a single user message produced from the provided prompts.
///
/// The returned `Messages` contains one `Content` with the user role whose parts are derived from `prompts`.
///
/// # Examples
///
/// ```
/// let msgs = from_user(vec!["Hello", "How are you?"]);
/// assert_eq!(msgs.len(), 1);
/// ```
pub fn from_user(prompts: Vec<&str>) -> Messages {
    Messages(vec![content::from_user(prompts)])
}

/// Creates a `Messages` wrapper containing a single system `Content` constructed from the provided prompts.
///
/// # Examples
///
/// ```
/// let msgs = from_system(vec!["You are a helpful assistant."]);
/// assert_eq!(msgs.len(), 1);
/// ```
pub fn from_system(prompts: Vec<&str>) -> Messages {
    Messages(vec![content::from_system(prompts)])
}

/// Create a `Messages` containing a single model `Content` constructed from the given prompts.
///
/// `prompts` are the textual parts used to build the model content in order.
///
/// # Returns
///
/// `Messages` containing one model `Content` assembled from `prompts`.
///
/// # Examples
///
/// ```
/// let msgs = from_model(vec!["Thinking...", "More details"]);
/// assert_eq!(msgs.len(), 1);
/// ```
pub fn from_model(prompts: Vec<&str>) -> Messages {
    Messages(vec![content::from_model(prompts)])
}

#[derive(Clone, Debug, Default)]
#[repr(transparent)]
pub struct Messages(pub Vec<Content>);

impl Messages {
    /// Appends a `Content` to this `Messages`, merging parts when the last entry has the same role.
    ///
    /// If the provided `content` has the same role as the last stored `Content`, its parts are appended to that last `Content` instead of adding a new entry.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut msgs = Messages(Vec::new());
    /// msgs.push(content::from_system(vec!["first"]));
    /// msgs.push(content::from_system(vec!["second"]));
    /// assert_eq!(msgs.len(), 1);
    /// ```
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

    /// Appends all `Content` items from `messages` into this `Messages` in order.
    ///
    /// This moves the contained `Content` values out of `messages` and into `self`.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::messages::content;
    /// use crate::messages::Messages;
    ///
    /// let mut a = Messages(vec![content::from_user(vec!["hello"])]);
    /// let b = Messages(vec![content::from_system(vec!["system"])]);
    /// a.extend(b);
    /// assert_eq!(a.len(), 2);
    /// ```
    pub fn extend(&mut self, messages: Messages) -> &mut Self {
        self.0.extend(messages.0);
        self
    }

    /// Get the number of content items in the `Messages`.
    ///
    /// # Returns
    ///
    /// The number of `Content` elements contained in this `Messages`.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::messages::Messages;
    ///
    /// let msgs = Messages(Vec::new());
    /// assert_eq!(msgs.len(), 0);
    /// ```
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Gets the last content item in the messages collection, if any.
    ///
    /// # Returns
    ///
    /// `Some(&Content)` containing the last content item, or `None` if the collection is empty.
    ///
    /// # Examples
    ///
    /// ```
    /// let msgs = from_user(vec!["hello"]);
    /// assert!(msgs.last().is_some());
    /// ```
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
            ..Content::default()
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
            ..Content::default()
        };

        messages.push(model_content);
        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn test_messages_extend() {
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["First"]));

        let mut more_messages = Messages::default();
        more_messages.push(content::from_system(vec!["System"]));
        more_messages.push(content::from_model(vec!["Model"]));

        messages.extend(more_messages);
        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn test_messages_last() {
        let mut messages = Messages::default();
        assert!(messages.last().is_none());

        let user_content = content::from_user(vec!["User message"]);
        messages.push(user_content.clone());

        assert_eq!(messages.last(), Some(&user_content));
    }

    #[test]
    fn test_messages_default() {
        let messages = Messages::default();
        assert_eq!(messages.len(), 0);
        assert!(messages.last().is_none());
    }

    #[test]
    fn test_messages_clone() {
        let mut messages1 = Messages::default();
        messages1.push(content::from_user(vec!["Test"]));

        let messages2 = messages1.clone();
        assert_eq!(messages1.len(), messages2.len());
    }

    #[test]
    fn test_messages_push_same_role_extends_parts() {
        let mut messages = Messages::default();
        let user_content1 = content::from_user(vec!["First message"]);
        let user_content2 = content::from_user(vec!["Second message"]);

        messages.push(user_content1);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.last().unwrap().parts.len(), 1);

        messages.push(user_content2);
        assert_eq!(messages.len(), 1); // Should still be 1 message
        assert_eq!(messages.last().unwrap().parts.len(), 2); // But with 2 parts
    }

    #[test]
    fn test_messages_push_different_roles_adds_new() {
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["User"]));
        messages.push(content::from_system(vec!["System"]));

        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_from_user_function() {
        let messages = from_user(vec!["Hello"]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.last().unwrap().role, RoleEnum::User);
    }

    #[test]
    fn test_from_system_function() {
        let messages = from_system(vec!["System prompt"]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.last().unwrap().role, RoleEnum::System);
    }

    #[test]
    fn test_from_model_function() {
        let messages = from_model(vec!["Model response"]);
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages.last().unwrap().complete_reason,
            CompleteReasonEnum::Stop
        );
    }

    #[test]
    fn test_from_user_with_multiple_prompts() {
        let messages = from_user(vec!["First", "Second", "Third"]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.last().unwrap().parts.len(), 3);
    }

    #[test]
    fn test_from_system_with_empty_prompts() {
        let messages = from_system(vec![]);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.last().unwrap().parts.len(), 0);
    }

    #[test]
    fn test_messages_chaining_push() {
        let mut messages = Messages::default();
        messages
            .push(content::from_user(vec!["First"]))
            .push(content::from_system(vec!["Second"]));

        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_messages_extend_empty() {
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["Existing"]));

        let empty_messages = Messages::default();
        messages.extend(empty_messages);

        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_messages_with_function_calls() {
        let mut messages = Messages::default();
        let thinking_part = PartEnum::from_reasoning("Thinking...");
        let fc = tools_rs::FunctionCall::new("test".to_string(), serde_json::json!({}));
        let fc_part = PartEnum::from_function_call(fc);

        let model_content = Content {
            parts: Parts(vec![thinking_part, fc_part]),
            role: RoleEnum::Model,
            complete_reason: CompleteReasonEnum::None,
            ..Content::default()
        };

        messages.push(model_content);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages.last().unwrap().parts.len(), 2);
    }

    #[test]
    fn test_messages_alternating_roles() {
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["User 1"]));
        messages.push(content::from_model(vec!["Model 1"]));
        messages.push(content::from_user(vec!["User 2"]));
        messages.push(content::from_model(vec!["Model 2"]));

        assert_eq!(messages.len(), 4);
    }

    #[test]
    fn test_messages_multiple_same_role_consolidation() {
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["Part 1"]));
        messages.push(content::from_user(vec!["Part 2"]));
        messages.push(content::from_user(vec!["Part 3"]));

        assert_eq!(messages.len(), 1);
        assert_eq!(messages.last().unwrap().parts.len(), 3);
    }
}
