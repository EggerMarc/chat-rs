use serde_json::Value;

pub trait GeminiNativeTool: Send + Sync {
    fn tool_key(&self) -> &'static str;
    /// Indicates whether the tool should be treated as a search tool.
    ///
    /// By default this returns `false`; implementors may override to mark a tool as a search tool.
    ///
    /// # Examples
    ///
    /// ```
    /// struct MyTool;
    ///
    /// impl crate::GeminiNativeTool for MyTool {}
    ///
    /// let t = MyTool;
    /// assert!(!t.is_search());
    /// ```
    fn is_search(&self) -> bool {
        false
    }
    fn to_tool_declaration(&self) -> Value;
    fn to_tool_config(&self) -> Option<(String, Value)>;
    fn clone_box(&self) -> Box<dyn GeminiNativeTool>;
}

impl Clone for Box<dyn GeminiNativeTool> {
    /// Returns a boxed clone of the underlying `GeminiNativeTool`.
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
