pub(crate) mod code_execution;
pub(crate) mod google_maps;
pub(crate) mod google_search;

use serde_json::Value;

pub trait GeminiNativeTool: Send + Sync {
    fn tool_key(&self) -> &'static str;
    fn is_search(&self) -> bool {
        false
    }
    fn to_tool_declaration(&self) -> Value;
    fn to_tool_config(&self) -> Option<(String, Value)>;
    fn clone_box(&self) -> Box<dyn GeminiNativeTool>;
}

impl Clone for Box<dyn GeminiNativeTool> {
    /// Create a boxed clone of the underlying `GeminiNativeTool`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let boxed: Box<dyn GeminiNativeTool> = /* create a tool */ unimplemented!();
    /// let cloned = boxed.clone();
    /// ```
    fn clone(&self) -> Box<dyn GeminiNativeTool> {
        self.clone_box()
    }
}
