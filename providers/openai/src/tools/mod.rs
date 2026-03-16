pub(crate) mod web_search;

use serde_json::Value;

pub trait OpenAINativeTool: Send + Sync {
    fn tool_key(&self) -> &'static str;
    fn is_search(&self) -> bool {
        false
    }
    fn to_tool_declaration(&self) -> Value;
    fn to_tool_config(&self) -> Option<(String, Value)>;
    fn clone_box(&self) -> Box<dyn OpenAINativeTool>;
}

impl Clone for Box<dyn OpenAINativeTool> {
    /// Create a boxed clone of the underlying `OpenAINativeTool`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let boxed: Box<dyn OpenAINativeTool> = /* create a tool */ unimplemented!();
    /// let cloned = boxed.clone();
    /// ```
    fn clone(&self) -> Box<dyn OpenAINativeTool> {
        self.clone_box()
    }
}
