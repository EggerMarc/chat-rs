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
    pub specific: HashMap<String, Value>, // TODO: rename to smth else
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_metadata_default() {
        let metadata = Metadata::default();
        assert!(metadata.id.is_none());
        assert!(metadata.model_slug.is_none());
        assert!(metadata.system_fingerprint.is_none());
        assert_eq!(metadata.usage.total_tokens, 0);
        assert!(metadata.duration_ms.is_none());
        assert!(metadata.specific.is_empty());
        assert!(metadata.created_at.is_none());
    }

    #[test]
    fn test_metadata_with_id() {
        let metadata = Metadata {
            id: Some("test-id-123".to_string()),
            ..Default::default()
        };
        assert_eq!(metadata.id.as_ref().unwrap(), "test-id-123");
    }

    #[test]
    fn test_metadata_with_model_slug() {
        let metadata = Metadata {
            model_slug: Some("gpt-4".to_string()),
            ..Default::default()
        };
        assert_eq!(metadata.model_slug.as_ref().unwrap(), "gpt-4");
    }

    #[test]
    fn test_metadata_with_usage() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            reasoning_tokens: 10,
            cache_creation_input_tokens: 5,
            cache_read_input_tokens: 3,
        };
        let metadata = Metadata {
            usage: usage.clone(),
            ..Default::default()
        };
        assert_eq!(metadata.usage.input_tokens, 100);
        assert_eq!(metadata.usage.output_tokens, 50);
        assert_eq!(metadata.usage.total_tokens, 150);
        assert_eq!(metadata.usage.reasoning_tokens, 10);
    }

    #[test]
    fn test_metadata_with_duration() {
        let metadata = Metadata {
            duration_ms: Some(1500),
            ..Default::default()
        };
        assert_eq!(metadata.duration_ms.unwrap(), 1500);
    }

    #[test]
    fn test_metadata_with_specific_data() {
        let mut specific = HashMap::new();
        specific.insert("safety_ratings".to_string(), json!(["safe", "filtered"]));
        specific.insert(
            "citation_metadata".to_string(),
            json!({"source": "wikipedia"}),
        );

        let metadata = Metadata {
            specific,
            ..Default::default()
        };

        assert_eq!(metadata.specific.len(), 2);
        assert!(metadata.specific.contains_key("safety_ratings"));
        assert!(metadata.specific.contains_key("citation_metadata"));
    }

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
        let mut specific = HashMap::new();
        specific.insert("key".to_string(), json!("value"));

        let metadata = Metadata {
            id: Some("test-123".to_string()),
            model_slug: Some("model-v1".to_string()),
            system_fingerprint: Some("fp_xyz".to_string()),
            usage: Usage {
                input_tokens: 10,
                output_tokens: 20,
                total_tokens: 30,
                reasoning_tokens: 5,
                cache_creation_input_tokens: 2,
                cache_read_input_tokens: 1,
            },
            duration_ms: Some(500),
            specific,
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
        let mut specific = HashMap::new();
        specific.insert("field1".to_string(), json!(42));
        specific.insert("field2".to_string(), json!("text"));

        let metadata = Metadata {
            id: Some("full-test".to_string()),
            model_slug: Some("gemini-pro".to_string()),
            system_fingerprint: Some("fp_full".to_string()),
            usage: Usage {
                input_tokens: 200,
                output_tokens: 100,
                total_tokens: 300,
                reasoning_tokens: 50,
                cache_creation_input_tokens: 10,
                cache_read_input_tokens: 5,
            },
            duration_ms: Some(2000),
            specific,
            created_at: Some(9999999999),
        };

        assert!(metadata.id.is_some());
        assert!(metadata.model_slug.is_some());
        assert!(metadata.system_fingerprint.is_some());
        assert_eq!(metadata.usage.total_tokens, 300);
        assert!(metadata.duration_ms.is_some());
        assert_eq!(metadata.specific.len(), 2);
        assert!(metadata.created_at.is_some());
    }
}
