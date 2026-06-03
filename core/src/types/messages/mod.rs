pub mod content;
pub mod embeddings;
pub mod file;
pub mod parts;
pub mod reasoning;
pub mod text;
pub mod tool;

use content::Content;
use parts::PartEnum;
use tool::Tool;
use tools_rs::CallId;

/// Create a `Messages` containing a single user message. Accepts any iterable
/// of items convertible into [`PartEnum`] — works for `vec!["hi", "world"]`
/// and for heterogeneous content built via [`parts!`](crate::parts).
pub fn from_user<I>(prompts: I) -> Messages
where
    I: IntoIterator,
    I::Item: Into<PartEnum>,
{
    Messages(vec![content::from_user(prompts)])
}

/// Create a `Messages` containing a single system message. See [`from_user`]
/// for accepted input shapes.
pub fn from_system<I>(prompts: I) -> Messages
where
    I: IntoIterator,
    I::Item: Into<PartEnum>,
{
    Messages(vec![content::from_system(prompts)])
}

/// Create a `Messages` containing a single model message. See [`from_user`]
/// for accepted input shapes.
pub fn from_model<I>(prompts: I) -> Messages
where
    I: IntoIterator,
    I::Item: Into<PartEnum>,
{
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
    /// ```ignore
    /// let mut msgs = Messages(Vec::new());
    /// msgs.push(content::from_system(vec!["first"]));
    /// msgs.push(content::from_system(vec!["second"]));
    /// assert_eq!(msgs.len(), 1);
    /// ```
    pub fn push(&mut self, content: Content) -> &mut Self {
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
    /// ```ignore
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
    /// ```ignore
    /// use crate::messages::Messages;
    ///
    /// let msgs = Messages(Vec::new());
    /// assert_eq!(msgs.len(), 0);
    /// ```
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Gets the last content item in the messages collection, if any.
    ///
    /// # Returns
    ///
    /// `Some(&Content)` containing the last content item, or `None` if the collection is empty.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let msgs = from_user(vec!["hello"]);
    /// assert!(msgs.last().is_some());
    /// ```
    pub fn last(&self) -> Option<&Content> {
        self.0.last()
    }

    /// Walk every `Content` and return the first `Tool` part whose id
    /// matches. Used by HITL callers to mutate a pending tool's status
    /// in place (`tool.approve(...)`, `tool.reject(...)`, etc) before
    /// calling `Chat::resume`.
    pub fn find_tool_mut(&mut self, id: &CallId) -> Option<&mut Tool> {
        for content in self.0.iter_mut() {
            for tool in content.parts.tools_mut() {
                if &tool.id == id {
                    return Some(tool);
                }
            }
        }
        None
    }

    pub fn find_tool(&self, id: &CallId) -> Option<&Tool> {
        for content in self.0.iter() {
            for tool in content.parts.tools() {
                if &tool.id == id {
                    return Some(tool);
                }
            }
        }
        None
    }
}
