use std::collections::HashMap;

use serde_json::Value;

#[derive(Debug, Clone, Default)]
pub struct ChatOptions {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub metadata: HashMap<String, Value>, // provider-specific extensions
}
