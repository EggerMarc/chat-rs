pub mod usage;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use usage::Usage;

#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Metadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_slug: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,

    #[serde(default)]
    pub usage: Usage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,

    /// Provider-specific arbitrary data.
    /// Gemini "safetyRatings", "citationMetadata", OpenAI "system_fingerprint", etc.
    /// key = "safety_ratings", value = json!([...])
    #[serde(default)]
    pub specific: HashMap<String, Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
}
