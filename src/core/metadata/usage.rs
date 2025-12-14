use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
    #[serde(default)]
    pub reasoning_tokens: usize,
    #[serde(default)]
    pub cache_creation_input_tokens: usize,
    #[serde(default)]
    pub cache_read_input_tokens: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_default() {
        let usage = Usage::default();
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
        assert_eq!(usage.reasoning_tokens, 0);
        assert_eq!(usage.cache_creation_input_tokens, 0);
        assert_eq!(usage.cache_read_input_tokens, 0);
    }

    #[test]
    fn test_usage_with_basic_tokens() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            ..Default::default()
        };
        assert_eq!(usage.input_tokens, 100);
        assert_eq!(usage.output_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn test_usage_with_reasoning_tokens() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 170,
            reasoning_tokens: 20,
            ..Default::default()
        };
        assert_eq!(usage.reasoning_tokens, 20);
        assert_eq!(usage.total_tokens, 170);
    }

    #[test]
    fn test_usage_with_cache_tokens() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            cache_creation_input_tokens: 30,
            cache_read_input_tokens: 20,
            ..Default::default()
        };
        assert_eq!(usage.cache_creation_input_tokens, 30);
        assert_eq!(usage.cache_read_input_tokens, 20);
    }

    #[test]
    fn test_usage_with_all_fields() {
        let usage = Usage {
            input_tokens: 1000,
            output_tokens: 500,
            total_tokens: 1550,
            reasoning_tokens: 30,
            cache_creation_input_tokens: 15,
            cache_read_input_tokens: 5,
        };
        assert_eq!(usage.input_tokens, 1000);
        assert_eq!(usage.output_tokens, 500);
        assert_eq!(usage.total_tokens, 1550);
        assert_eq!(usage.reasoning_tokens, 30);
        assert_eq!(usage.cache_creation_input_tokens, 15);
        assert_eq!(usage.cache_read_input_tokens, 5);
    }

    #[test]
    fn test_usage_zero_tokens() {
        let usage = Usage {
            input_tokens: 0,
            output_tokens: 0,
            total_tokens: 0,
            reasoning_tokens: 0,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.total_tokens, 0);
    }

    #[test]
    fn test_usage_serialization() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            reasoning_tokens: 10,
            cache_creation_input_tokens: 5,
            cache_read_input_tokens: 3,
        };

        let serialized = serde_json::to_string(&usage).unwrap();
        let deserialized: Usage = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(usage.input_tokens, deserialized.input_tokens);
        assert_eq!(usage.output_tokens, deserialized.output_tokens);
        assert_eq!(usage.total_tokens, deserialized.total_tokens);
        assert_eq!(usage.reasoning_tokens, deserialized.reasoning_tokens);
    }

    #[test]
    fn test_usage_clone() {
        let usage = Usage {
            input_tokens: 75,
            output_tokens: 25,
            total_tokens: 100,
            ..Default::default()
        };
        
        let cloned = usage.clone();
        assert_eq!(usage.input_tokens, cloned.input_tokens);
        assert_eq!(usage.output_tokens, cloned.output_tokens);
        assert_eq!(usage.total_tokens, cloned.total_tokens);
    }

    #[test]
    fn test_usage_partial_eq() {
        let usage1 = Usage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            ..Default::default()
        };
        
        let usage2 = Usage {
            input_tokens: 100,
            output_tokens: 50,
            total_tokens: 150,
            ..Default::default()
        };
        
        let usage3 = Usage {
            input_tokens: 200,
            output_tokens: 100,
            total_tokens: 300,
            ..Default::default()
        };
        
        assert_eq!(usage1, usage2);
        assert_ne!(usage1, usage3);
    }

    #[test]
    fn test_usage_large_numbers() {
        let usage = Usage {
            input_tokens: 1_000_000,
            output_tokens: 500_000,
            total_tokens: 1_500_000,
            reasoning_tokens: 50_000,
            cache_creation_input_tokens: 10_000,
            cache_read_input_tokens: 5_000,
        };
        assert_eq!(usage.input_tokens, 1_000_000);
        assert_eq!(usage.total_tokens, 1_500_000);
    }
}
