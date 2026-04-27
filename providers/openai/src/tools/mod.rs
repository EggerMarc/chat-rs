pub(crate) mod image_generation;
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
    fn clone(&self) -> Box<dyn OpenAINativeTool> {
        self.clone_box()
    }
}
