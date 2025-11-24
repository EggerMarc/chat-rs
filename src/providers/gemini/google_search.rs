use serde_json::{Value, json};

use crate::gemini::lib::GeminiNativeTool;

#[derive(Clone)]
pub struct GoogleSearchTool {
    pub dynamic_threshold: Option<f32>,
}

impl GeminiNativeTool for GoogleSearchTool {
    fn tool_key(&self) -> &'static str {
        "googleSearch"
    }

    fn is_search(&self) -> bool {
        true
    }

    fn to_tool_declaration(&self) -> Value {
        json!({ "googleSearch": {} })
    }

    fn to_tool_config(&self) -> Option<(String, Value)> {
        self.dynamic_threshold.map(|t| {
            (
                "googleSearchRetrieval".to_string(),
                json!({
                    "dynamicRetrievalConfig": {
                        "mode": "MODE_DYNAMIC",
                        "dynamicThreshold": t
                    }
                }),
            )
        })
    }

    fn clone_box(&self) -> Box<dyn GeminiNativeTool> {
        Box::new(self.clone())
    }
}
