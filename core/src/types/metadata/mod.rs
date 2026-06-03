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
    pub provider_specific: HashMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
}

impl Metadata {
    pub fn add_usage(&mut self, usage: &Usage) -> &Self {
        self.usage.output_tokens += usage.output_tokens;
        self.usage.input_tokens += usage.input_tokens;
        self.usage.total_tokens += usage.total_tokens;
        self
    }

    pub fn extend(&mut self, metadata: &Metadata) -> &Self {
        self.add_usage(&metadata.usage);

        if let Some(other) = metadata.duration_ms {
            let current = self.duration_ms.get_or_insert(0);
            *current += other;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_metadata_with_created_at() {
        let timestamp = 1640000000u64;
        let metadata = Metadata {
            created_at: Some(timestamp),
            ..Default::default()
        };
        assert_eq!(metadata.created_at.unwrap(), timestamp);
    }

    #[test]
    fn test_metadata_with_system_fingerprint() {
        let metadata = Metadata {
            system_fingerprint: Some("fp_abc123".to_string()),
            ..Default::default()
        };
        assert_eq!(metadata.system_fingerprint.as_ref().unwrap(), "fp_abc123");
    }

    #[test]
    fn test_metadata_serialization() {
        let mut provider_specific = HashMap::new();
        provider_specific.insert("key".to_string(), json!("value"));

        let metadata = Metadata {
            id: Some("test-123".to_string()),
            model_slug: Some("model-v1".to_string()),
            system_fingerprint: Some("fp_xyz".to_string()),
            usage: Usage {
                input_tokens: 10,
                output_tokens: 20,
                total_tokens: 30,
            },
            duration_ms: Some(500),
            provider_specific,
            created_at: Some(1234567890),
        };

        let serialized = serde_json::to_string(&metadata).unwrap();
        let deserialized: Metadata = serde_json::from_str(&serialized).unwrap();

        assert_eq!(metadata.id, deserialized.id);
        assert_eq!(metadata.model_slug, deserialized.model_slug);
        assert_eq!(metadata.usage.total_tokens, deserialized.usage.total_tokens);
    }

    #[test]
    fn test_metadata_clone() {
        let metadata = Metadata {
            id: Some("clone-test".to_string()),
            usage: Usage {
                total_tokens: 100,
                ..Default::default()
            },
            ..Default::default()
        };

        let cloned = metadata.clone();
        assert_eq!(metadata.id, cloned.id);
        assert_eq!(metadata.usage.total_tokens, cloned.usage.total_tokens);
    }

    #[test]
    fn test_metadata_partial_eq() {
        let metadata1 = Metadata {
            id: Some("same".to_string()),
            ..Default::default()
        };

        let metadata2 = Metadata {
            id: Some("same".to_string()),
            ..Default::default()
        };

        let metadata3 = Metadata {
            id: Some("different".to_string()),
            ..Default::default()
        };

        assert_eq!(metadata1, metadata2);
        assert_ne!(metadata1, metadata3);
    }

    #[test]
    fn test_metadata_with_all_fields() {
        let mut provider_specific = HashMap::new();
        provider_specific.insert("field1".to_string(), json!(42));
        provider_specific.insert("field2".to_string(), json!("text"));

        let metadata = Metadata {
            id: Some("full-test".to_string()),
            model_slug: Some("gemini-pro".to_string()),
            system_fingerprint: Some("fp_full".to_string()),
            usage: Usage {
                input_tokens: 200,
                output_tokens: 100,
                total_tokens: 300,
            },
            duration_ms: Some(2000),
            provider_specific,
            created_at: Some(9999999999),
        };

        assert!(metadata.id.is_some());
        assert!(metadata.model_slug.is_some());
        assert!(metadata.system_fingerprint.is_some());
        assert_eq!(metadata.usage.total_tokens, 300);
        assert!(metadata.duration_ms.is_some());
        assert_eq!(metadata.provider_specific.len(), 2);
        assert!(metadata.created_at.is_some());
    }
}
