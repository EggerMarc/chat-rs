use serde::Serialize;
use serde_json::{Value, json};

use crate::tools::OpenAINativeTool;

#[derive(Serialize, Clone)]
pub enum SearchContextSizeEnum {
    Low,
    High,
    Medium,
}

#[derive(Serialize, Clone)]
pub struct UserLocation {
    city: Option<String>,
    region: Option<String>,
    country: Option<String>,
    timezone: Option<String>,
}

#[derive(Clone)]
pub struct WebSearchTool {
    pub context_size: Option<SearchContextSizeEnum>,
    pub user_location: Option<UserLocation>,
}

impl OpenAINativeTool for WebSearchTool {
    fn tool_key(&self) -> &'static str {
        "web_search"
    }

    fn to_tool_declaration(&self) -> Value {
        json!({
            "type": "web_search",
            "web_search": {
                "context_size": self.context_size,
                "user_location": self.user_location
            }
        })
    }

    fn to_tool_config(&self) -> Option<(String, Value)> {
        None
    }

    fn clone_box(&self) -> Box<dyn OpenAINativeTool> {
        Box::new(self.clone())
    }
}
