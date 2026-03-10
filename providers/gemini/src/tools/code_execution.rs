use serde_json::{Value, json};

use crate::gemini::lib::GeminiNativeTool;

#[derive(Clone)]
pub struct CodeExecutionTool;

impl GeminiNativeTool for CodeExecutionTool {
    /// Returns this tool's stable identifier key.
    ///
    /// The returned string is the static tool key used to declare the tool: `"codeExecution"`.
    ///
    /// # Examples
    ///
    /// ```
    /// let tool = CodeExecutionTool;
    /// assert_eq!(tool.tool_key(), "codeExecution");
    /// ```
    fn tool_key(&self) -> &'static str {
        "codeExecution"
    }

    /// Produces the JSON declaration used to register the `codeExecution` native tool.
    ///
    /// # Examples
    ///
    /// ```
    /// use serde_json::json;
    /// let decl = CodeExecutionTool.to_tool_declaration();
    /// assert_eq!(decl, json!({ "codeExecution": {} }));
    /// ```
    fn to_tool_declaration(&self) -> Value {
        json!({ "codeExecution": {} })
    }

    /// Return the tool's configuration as an optional named JSON value.
    ///
    /// For tools that expose configuration, this should be `Some((name, value))` where `name` is
    /// the configuration key and `value` is a JSON object describing configuration details.
    /// For tools without configuration, this returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// let tool = crate::providers::gemini::code_execution::CodeExecutionTool;
    /// assert!(tool.to_tool_config().is_none());
    /// ```
    fn to_tool_config(&self) -> Option<(String, Value)> {
        None
    }

    /// Create a boxed clone of the tool.
    ///
    /// Returns a boxed trait object containing a clone of this tool.
    ///
    /// # Examples
    ///
    /// ```
    /// let tool = CodeExecutionTool {};
    /// let boxed: Box<dyn GeminiNativeTool> = tool.clone_box();
    /// ```
    fn clone_box(&self) -> Box<dyn GeminiNativeTool> {
        Box::new(self.clone())
    }
}