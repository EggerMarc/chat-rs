use std::fmt::Display;

use crate::core::messages::text::Text;
use serde::{Deserialize, Serialize};
use tools_rs::{CallId, FunctionCall, FunctionResponse};

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[repr(transparent)]
pub struct Parts(pub Vec<PartEnum>);

impl Parts {
    pub fn text_response(&self) -> Option<&Text> {
        self.0
            .iter()
            .filter_map(|res| match res {
                PartEnum::Text(text) => Some(text),
                _ => None,
            })
            .next()
    }

    pub fn length(&self) -> usize {
        self.0.len()
    }

    pub fn structured_response(&self) -> Option<&serde_json::Value> {
        self.0
            .iter()
            .filter_map(|res| match res {
                PartEnum::Structured(value) => Some(value),
                _ => None,
            })
            .next()
    }

    pub fn push(&mut self, part: PartEnum) -> &mut Self {
        self.0.push(part);
        self
    }

    pub fn extend(&mut self, parts: Parts) -> &mut Self {
        self.0.extend(parts.0);
        self
    }

    pub fn last(&self) -> Option<&PartEnum> {
        self.0.last()
    }

    pub fn text_parts(&self) -> impl Iterator<Item = &Text> + '_ {
        self.0.iter().filter_map(|p| match p {
            PartEnum::Text(text) => Some(text),
            _ => None,
        })
    }

    pub fn function_calls(&self) -> impl Iterator<Item = &FunctionCall> + '_ {
        self.0.iter().filter_map(|p| match p {
            PartEnum::FunctionCall(fc) => Some(fc),
            _ => None,
        })
    }

    pub fn function_responses(&self) -> impl Iterator<Item = &FunctionResponse> + '_ {
        self.0.iter().filter_map(|p| match p {
            PartEnum::FunctionResponse(fr) => Some(fr),
            _ => None,
        })
    }

    pub fn function_response(&self, id: impl Into<CallId>) -> Option<&FunctionResponse> {
        let call_id: CallId = id.into();
        self.0
            .iter()
            .filter_map(|f| match f {
                PartEnum::FunctionResponse(fr) => {
                    if let Some(fr_id) = &fr.id
                        && *fr_id == call_id
                    {
                        Some(fr)
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .next()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum PartEnum {
    Reasoning(Text),
    Text(Text),
    FunctionCall(FunctionCall),
    FunctionResponse(FunctionResponse),
    Structured(serde_json::Value),
    /*
    Document,
    */
}

impl Default for PartEnum {
    fn default() -> Self {
        PartEnum::Text(Text("".to_string()))
    }
}

impl PartEnum {
    pub fn function_call(&self) -> Option<FunctionCall> {
        match self {
            PartEnum::FunctionCall(fc) => Some(fc.clone()),
            _ => None,
        }
    }

    pub fn function_response(&self) -> Option<FunctionResponse> {
        match self {
            PartEnum::FunctionResponse(fr) => Some(fr.clone()),
            _ => None,
        }
    }

    pub fn text(&self) -> Option<Text> {
        match self {
            PartEnum::Text(text) => Some(text.clone()),
            _ => None,
        }
    }

    pub fn reasoning(&self) -> Option<Text> {
        match self {
            PartEnum::Reasoning(text) => Some(text.clone()),
            _ => None,
        }
    }

    pub fn structured(&self) -> Option<serde_json::Value> {
        match self {
            PartEnum::Structured(value) => Some(value.clone()),
            _ => None,
        }
    }

    pub fn from_reasoning(s: impl Into<String>) -> PartEnum {
        PartEnum::Reasoning(Text::new(s))
    }

    pub fn from_text(s: impl Into<String>) -> PartEnum {
        PartEnum::Text(Text::new(s))
    }

    pub fn from_function_call(fc: FunctionCall) -> PartEnum {
        PartEnum::FunctionCall(fc)
    }
    pub fn from_function_response(fr: FunctionResponse) -> PartEnum {
        PartEnum::FunctionResponse(fr)
    }

    pub fn from_structured(value: serde_json::Value) -> PartEnum {
        PartEnum::Structured(value)
    }
}

impl Display for PartEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PartEnum::Structured(value) => write!(f, "{}", value),
            PartEnum::Reasoning(text) => write!(f, "{}", text),
            PartEnum::Text(text) => write!(f, "{}", text),
            PartEnum::FunctionCall(fc) => write!(f, "{}", fc.name),
            PartEnum::FunctionResponse(fr) => write!(f, "{}", fr.name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parts_default() {
        let parts = Parts::default();
        assert_eq!(parts.length(), 0);
        assert!(parts.0.is_empty());
    }

    #[test]
    fn test_parts_push_text() {
        let mut parts = Parts::default();
        let text_part = PartEnum::from_text("Hello".to_string());
        parts.push(text_part.clone());
        
        assert_eq!(parts.length(), 1);
        assert_eq!(parts.0[0], text_part);
    }

    #[test]
    fn test_parts_push_multiple() {
        let mut parts = Parts::default();
        parts.push(PartEnum::from_text("First".to_string()));
        parts.push(PartEnum::from_text("Second".to_string()));
        parts.push(PartEnum::from_reasoning("Thinking".to_string()));
        
        assert_eq!(parts.length(), 3);
    }

    #[test]
    fn test_parts_extend() {
        let mut parts1 = Parts::default();
        parts1.push(PartEnum::from_text("Part1".to_string()));
        
        let mut parts2 = Parts::default();
        parts2.push(PartEnum::from_text("Part2".to_string()));
        parts2.push(PartEnum::from_reasoning("Reasoning".to_string()));
        
        parts1.extend(parts2.clone());
        assert_eq!(parts1.length(), 3);
    }

    #[test]
    fn test_parts_last_text() {
        let mut parts = Parts::default();
        parts.push(PartEnum::from_text("First".to_string()));
        parts.push(PartEnum::from_text("Second".to_string()));
        
        let last = parts.last_text();
        assert!(last.is_some());
        assert_eq!(last.unwrap().to_string(), "Second");
    }

    #[test]
    fn test_parts_last_text_empty() {
        let parts = Parts::default();
        assert!(parts.last_text().is_none());
    }

    #[test]
    fn test_parts_last_text_non_text_part() {
        let mut parts = Parts::default();
        parts.push(PartEnum::from_reasoning("Thinking".to_string()));
        
        let last = parts.last_text();
        assert!(last.is_none());
    }

    #[test]
    fn test_parts_filter_text_only() {
        let mut parts = Parts::default();
        parts.push(PartEnum::from_text("Text1".to_string()));
        parts.push(PartEnum::from_reasoning("Reasoning".to_string()));
        parts.push(PartEnum::from_text("Text2".to_string()));
        
        let text_parts = parts.filter_text_only();
        assert_eq!(text_parts.length(), 2);
    }

    #[test]
    fn test_parts_filter_text_only_empty() {
        let mut parts = Parts::default();
        parts.push(PartEnum::from_reasoning("Only reasoning".to_string()));
        
        let text_parts = parts.filter_text_only();
        assert_eq!(text_parts.length(), 0);
    }

    #[test]
    fn test_parts_function_calls() {
        let mut parts = Parts::default();
        let fc1 = FunctionCall::new("func1".to_string(), json!({"param": "value1"}));
        let fc2 = FunctionCall::new("func2".to_string(), json!({"param": "value2"}));
        
        parts.push(PartEnum::from_function_call(fc1.clone()));
        parts.push(PartEnum::from_text("Some text".to_string()));
        parts.push(PartEnum::from_function_call(fc2.clone()));
        
        let fcs = parts.function_calls();
        assert_eq!(fcs.len(), 2);
        assert_eq!(fcs[0].name, "func1");
        assert_eq!(fcs[1].name, "func2");
    }

    #[test]
    fn test_parts_function_calls_empty() {
        let parts = Parts::default();
        let fcs = parts.function_calls();
        assert_eq!(fcs.len(), 0);
    }

    #[test]
    fn test_parts_function_call_by_id() {
        let mut parts = Parts::default();
        let fc = FunctionCall::new("test_func".to_string(), json!({"key": "value"}));
        let call_id = fc.id.clone().unwrap();
        
        parts.push(PartEnum::from_function_call(fc.clone()));
        parts.push(PartEnum::from_text("Text".to_string()));
        
        let found = parts.function_call(call_id.clone());
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "test_func");
    }

    #[test]
    fn test_parts_function_call_by_id_not_found() {
        let mut parts = Parts::default();
        let fc = FunctionCall::new("test_func".to_string(), json!({"key": "value"}));
        parts.push(PartEnum::from_function_call(fc));
        
        let missing_id = CallId::new();
        let found = parts.function_call(missing_id);
        assert!(found.is_none());
    }

    #[test]
    fn test_parts_function_response() {
        let mut parts = Parts::default();
        let fc = FunctionCall::new("test".to_string(), json!({}));
        let call_id = fc.id.clone().unwrap();
        let fr = FunctionResponse {
            id: call_id.clone(),
            name: "test".to_string(),
            result: json!({"status": "success"}),
        };
        
        parts.push(PartEnum::from_function_response(fr.clone()));
        
        let found = parts.function_response(call_id);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "test");
    }

    #[test]
    fn test_parts_function_response_not_found() {
        let parts = Parts::default();
        let missing_id = CallId::new();
        let found = parts.function_response(missing_id);
        assert!(found.is_none());
    }

    #[test]
    fn test_part_enum_from_text() {
        let part = PartEnum::from_text("Test text".to_string());
        match part {
            PartEnum::Text(text) => assert_eq!(text.to_string(), "Test text"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_part_enum_from_reasoning() {
        let part = PartEnum::from_reasoning("Thinking...".to_string());
        match part {
            PartEnum::Reasoning(text) => assert_eq!(text.to_string(), "Thinking..."),
            _ => panic!("Expected Reasoning variant"),
        }
    }

    #[test]
    fn test_part_enum_from_function_call() {
        let fc = FunctionCall::new("my_func".to_string(), json!({"arg": 42}));
        let part = PartEnum::from_function_call(fc.clone());
        
        match part {
            PartEnum::FunctionCall(call) => {
                assert_eq!(call.name, "my_func");
                assert_eq!(call.id, fc.id);
            },
            _ => panic!("Expected FunctionCall variant"),
        }
    }

    #[test]
    fn test_part_enum_from_function_response() {
        let call_id = CallId::new();
        let fr = FunctionResponse {
            id: call_id.clone(),
            name: "test_func".to_string(),
            result: json!({"result": "ok"}),
        };
        let part = PartEnum::from_function_response(fr.clone());
        
        match part {
            PartEnum::FunctionResponse(response) => {
                assert_eq!(response.name, "test_func");
                assert_eq!(response.id, call_id);
            },
            _ => panic!("Expected FunctionResponse variant"),
        }
    }

    #[test]
    fn test_parts_clone() {
        let mut parts1 = Parts::default();
        parts1.push(PartEnum::from_text("Test".to_string()));
        parts1.push(PartEnum::from_reasoning("Thinking".to_string()));
        
        let parts2 = parts1.clone();
        assert_eq!(parts1.length(), parts2.length());
        assert_eq!(parts1, parts2);
    }

    #[test]
    fn test_parts_equality() {
        let mut parts1 = Parts::default();
        parts1.push(PartEnum::from_text("Same".to_string()));
        
        let mut parts2 = Parts::default();
        parts2.push(PartEnum::from_text("Same".to_string()));
        
        assert_eq!(parts1, parts2);
    }

    #[test]
    fn test_parts_inequality() {
        let mut parts1 = Parts::default();
        parts1.push(PartEnum::from_text("Different1".to_string()));
        
        let mut parts2 = Parts::default();
        parts2.push(PartEnum::from_text("Different2".to_string()));
        
        assert_ne!(parts1, parts2);
    }

    #[test]
    fn test_parts_serialization() {
        let mut parts = Parts::default();
        parts.push(PartEnum::from_text("Serialize me".to_string()));
        
        let json = serde_json::to_string(&parts).unwrap();
        let deserialized: Parts = serde_json::from_str(&json).unwrap();
        
        assert_eq!(parts, deserialized);
    }

    #[test]
    fn test_part_enum_equality() {
        let part1 = PartEnum::from_text("Equal".to_string());
        let part2 = PartEnum::from_text("Equal".to_string());
        assert_eq!(part1, part2);
    }

    #[test]
    fn test_part_enum_inequality() {
        let part1 = PartEnum::from_text("A".to_string());
        let part2 = PartEnum::from_text("B".to_string());
        assert_ne!(part1, part2);
    }

    #[test]
    fn test_parts_mixed_content() {
        let mut parts = Parts::default();
        let fc = FunctionCall::new("func".to_string(), json!({}));
        let call_id = fc.id.clone().unwrap();
        let fr = FunctionResponse {
            id: call_id.clone(),
            name: "func".to_string(),
            result: json!({"done": true}),
        };
        
        parts.push(PartEnum::from_text("Start".to_string()));
        parts.push(PartEnum::from_reasoning("Thinking".to_string()));
        parts.push(PartEnum::from_function_call(fc));
        parts.push(PartEnum::from_function_response(fr));
        parts.push(PartEnum::from_text("End".to_string()));
        
        assert_eq!(parts.length(), 5);
        assert_eq!(parts.function_calls().len(), 1);
        assert!(parts.function_response(call_id).is_some());
        assert_eq!(parts.filter_text_only().length(), 2);
    }

    #[test]
    fn test_parts_empty_extend() {
        let mut parts = Parts::default();
        parts.push(PartEnum::from_text("Original".to_string()));
        
        let empty_parts = Parts::default();
        parts.extend(empty_parts);
        
        assert_eq!(parts.length(), 1);
    }

    #[test]
    fn test_parts_chaining() {
        let parts = Parts::default()
            .push(PartEnum::from_text("First".to_string()))
            .push(PartEnum::from_text("Second".to_string()))
            .clone();
        
        assert_eq!(parts.length(), 2);
    }
}
