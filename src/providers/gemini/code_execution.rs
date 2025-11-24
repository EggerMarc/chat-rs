use serde_json::{Value, json};

use crate::gemini::lib::GeminiNativeTool;

#[derive(Clone)]
pub struct CodeExecutionTool;

impl GeminiNativeTool for CodeExecutionTool {
    fn tool_key(&self) -> &'static str {
        "codeExecution"
    }

    fn to_tool_declaration(&self) -> Value {
        json!({ "codeExecution": {} })
    }

    fn to_tool_config(&self) -> Option<(String, Value)> {
        None
    }

    fn clone_box(&self) -> Box<dyn GeminiNativeTool> {
        Box::new(self.clone())
    }
}
