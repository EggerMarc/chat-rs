pub mod content;
pub mod embeddings;
pub mod file;
pub mod parts;
pub mod reasoning;
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
