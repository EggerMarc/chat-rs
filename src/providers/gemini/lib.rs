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
    fn clone(&self) -> Box<dyn GeminiNativeTool> {
        self.clone_box()
    }
}
