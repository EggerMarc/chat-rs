
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_structured_default() {
        let structured = Structured::default();
        assert!(structured.0.is_object());
        assert_eq!(structured.0.as_object().unwrap().len(), 0);
    }

    #[test]
    fn test_structured_from_value_object() {
        let value = json!({
            "name": "John",
            "age": 30,
            "active": true
        });
        let structured = Structured::from(value);
        
        assert_eq!(structured.0["name"], "John");
        assert_eq!(structured.0["age"], 30);
        assert_eq!(structured.0["active"], true);
    }

    #[test]
    fn test_structured_from_empty_object() {
        let value = json!({});
        let structured = Structured::from(value);
        assert!(structured.0.is_object());
        assert_eq!(structured.0.as_object().unwrap().len(), 0);
    }

    #[test]
    fn test_structured_nested_objects() {
        let value = json!({
            "user": {
                "name": "Alice",
                "details": {
                    "age": 25,
                    "city": "NYC"
                }
            }
        });
        let structured = Structured::from(value);
        
        assert!(structured.0["user"].is_object());
        assert_eq!(structured.0["user"]["name"], "Alice");
        assert_eq!(structured.0["user"]["details"]["age"], 25);
    }

    #[test]
    fn test_structured_with_arrays() {
        let value = json!({
            "numbers": [1, 2, 3, 4, 5],
            "strings": ["a", "b", "c"]
        });
        let structured = Structured::from(value);
        
        assert!(structured.0["numbers"].is_array());
        assert_eq!(structured.0["numbers"].as_array().unwrap().len(), 5);
        assert_eq!(structured.0["strings"][0], "a");
    }

    #[test]
    fn test_structured_clone() {
        let value = json!({"key": "value"});
        let structured1 = Structured::from(value);
        let structured2 = structured1.clone();
        
        assert_eq!(structured1.0, structured2.0);
    }

    #[test]
    fn test_structured_debug() {
        let value = json!({"test": "data"});
        let structured = Structured::from(value);
        let debug_string = format!("{:?}", structured);
        
        assert!(debug_string.contains("Structured"));
    }

    #[test]
    fn test_structured_equality() {
        let value1 = json!({"a": 1, "b": 2});
        let value2 = json!({"a": 1, "b": 2});
        
        let structured1 = Structured::from(value1);
        let structured2 = Structured::from(value2);
        
        assert_eq!(structured1, structured2);
    }

    #[test]
    fn test_structured_inequality() {
        let value1 = json!({"a": 1});
        let value2 = json!({"a": 2});
        
        let structured1 = Structured::from(value1);
        let structured2 = Structured::from(value2);
        
        assert_ne!(structured1, structured2);
    }

    #[test]
    fn test_structured_serialization() {
        let value = json!({
            "field1": "value1",
            "field2": 42
        });
        let structured = Structured::from(value);
        
        let json = serde_json::to_string(&structured).unwrap();
        let deserialized: Structured = serde_json::from_str(&json).unwrap();
        
        assert_eq!(structured, deserialized);
    }

    #[test]
    fn test_structured_with_null_values() {
        let value = json!({
            "present": "here",
            "absent": null
        });
        let structured = Structured::from(value);
        
        assert!(structured.0["absent"].is_null());
        assert_eq!(structured.0["present"], "here");
    }

    #[test]
    fn test_structured_with_boolean_values() {
        let value = json!({
            "isActive": true,
            "isDeleted": false
        });
        let structured = Structured::from(value);
        
        assert_eq!(structured.0["isActive"], true);
        assert_eq!(structured.0["isDeleted"], false);
    }

    #[test]
    fn test_structured_with_numeric_types() {
        let value = json!({
            "integer": 42,
            "float": 3.14,
            "negative": -10,
            "zero": 0
        });
        let structured = Structured::from(value);
        
        assert_eq!(structured.0["integer"], 42);
        assert_eq!(structured.0["float"], 3.14);
        assert_eq!(structured.0["negative"], -10);
        assert_eq!(structured.0["zero"], 0);
    }

    #[test]
    fn test_structured_complex_structure() {
        let value = json!({
            "user": {
                "id": 123,
                "name": "Alice",
                "roles": ["admin", "user"],
                "settings": {
                    "theme": "dark",
                    "notifications": true
                }
            },
            "metadata": {
                "created": "2024-01-01",
                "updated": "2024-01-15"
            }
        });
        let structured = Structured::from(value);
        
        assert_eq!(structured.0["user"]["id"], 123);
        assert_eq!(structured.0["user"]["roles"][0], "admin");
        assert_eq!(structured.0["user"]["settings"]["theme"], "dark");
        assert_eq!(structured.0["metadata"]["created"], "2024-01-01");
    }

    #[test]
    fn test_structured_empty_arrays() {
        let value = json!({
            "emptyArray": [],
            "data": "value"
        });
        let structured = Structured::from(value);
        
        assert!(structured.0["emptyArray"].is_array());
        assert_eq!(structured.0["emptyArray"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_structured_special_string_values() {
        let value = json!({
            "empty": "",
            "whitespace": "   ",
            "newlines": "line1\nline2",
            "unicode": "Hello 世界 🌍"
        });
        let structured = Structured::from(value);
        
        assert_eq!(structured.0["empty"], "");
        assert_eq!(structured.0["whitespace"], "   ");
        assert!(structured.0["newlines"].as_str().unwrap().contains('\n'));
        assert_eq!(structured.0["unicode"], "Hello 世界 🌍");
    }

    #[test]
    fn test_structured_large_numbers() {
        let value = json!({
            "large_int": 9999999999i64,
            "small_float": 0.0000001
        });
        let structured = Structured::from(value);
        
        assert_eq!(structured.0["large_int"], 9999999999i64);
        assert!(structured.0["small_float"].is_f64());
    }

    #[test]
    fn test_structured_mixed_array_types() {
        let value = json!({
            "mixed": [1, "two", true, null, {"key": "value"}]
        });
        let structured = Structured::from(value);
        
        let array = structured.0["mixed"].as_array().unwrap();
        assert_eq!(array.len(), 5);
        assert_eq!(array[0], 1);
        assert_eq!(array[1], "two");
        assert_eq!(array[2], true);
        assert!(array[3].is_null());
        assert!(array[4].is_object());
    }

    #[test]
    fn test_structured_key_with_special_chars() {
        let value = json!({
            "normal-key": "value1",
            "key with spaces": "value2",
            "key.with.dots": "value3",
            "key_with_underscores": "value4"
        });
        let structured = Structured::from(value);
        
        assert_eq!(structured.0["normal-key"], "value1");
        assert_eq!(structured.0["key with spaces"], "value2");
        assert_eq!(structured.0["key.with.dots"], "value3");
        assert_eq!(structured.0["key_with_underscores"], "value4");
    }
}
