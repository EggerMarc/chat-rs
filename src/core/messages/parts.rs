use crate::core::messages::text::Text;
use crate::messages::file::File;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::slice::{Iter, IterMut};
use tools_rs::{CallId, FunctionCall, FunctionResponse};

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[repr(transparent)]
pub struct Parts(pub Vec<PartEnum>);

// Immutable iterator
impl IntoIterator for Parts {
    type Item = PartEnum;
    type IntoIter = std::vec::IntoIter<PartEnum>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

// Borrowed iterator: &Parts
impl<'a> IntoIterator for &'a Parts {
    type Item = &'a PartEnum;
    type IntoIter = Iter<'a, PartEnum>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

// Mutable borrowed iterator: &mut Parts
impl<'a> IntoIterator for &'a mut Parts {
    type Item = &'a mut PartEnum;
    type IntoIter = IterMut<'a, PartEnum>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

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

    /// Get the number of parts contained in this `Parts` wrapper.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::core::messages::parts::{Parts, PartEnum};
    ///
    /// let parts = Parts(vec![PartEnum::from_text("hello")]);
    /// assert_eq!(parts.len(), 1);
    /// ```
    pub fn len(&self) -> usize {
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

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
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
    File(File),
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

    pub fn file(&self) -> Option<File> {
        match self {
            PartEnum::File(file) => Some(file.clone()),
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

    pub fn from_file(file: File) -> PartEnum {
        PartEnum::File(file)
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
            PartEnum::File(file) => write!(
                f,
                "{}",
                match file {
                    File::Url(url) => format!("{} {:?}", url.url, url.mimetype),
                    File::Bytes(raw) => raw.mimetype.to_string(),
                }
            ),
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
        assert_eq!(parts.len(), 0);
        assert!(parts.is_empty());
    }

    #[test]
    fn test_parts_push() {
        let mut parts = Parts::default();
        parts.push(PartEnum::from_text("Hello"));
        assert_eq!(parts.len(), 1);
        assert!(!parts.is_empty());
    }

    #[test]
    fn test_parts_push_chaining() {
        let mut parts = Parts::default();
        parts
            .push(PartEnum::from_text("First"))
            .push(PartEnum::from_text("Second"));
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn test_parts_extend() {
        let mut parts1 = Parts::default();
        parts1.push(PartEnum::from_text("First"));

        let mut parts2 = Parts::default();
        parts2.push(PartEnum::from_text("Second"));

        parts1.extend(parts2);
        assert_eq!(parts1.len(), 2);
    }

    #[test]
    fn test_parts_last() {
        let mut parts = Parts::default();
        assert!(parts.last().is_none());

        parts.push(PartEnum::from_text("First"));
        parts.push(PartEnum::from_text("Second"));

        let last = parts.last().unwrap();
        assert_eq!(last.text().unwrap().as_str(), "Second");
    }

    #[test]
    fn test_parts_text_response() {
        let mut parts = Parts::default();
        parts.push(PartEnum::from_reasoning("Thinking"));
        parts.push(PartEnum::from_text("Response"));

        let text = parts.text_response().unwrap();
        assert_eq!(text.as_str(), "Response");
    }

    #[test]
    fn test_parts_text_response_none() {
        let mut parts = Parts::default();
        parts.push(PartEnum::from_reasoning("Only reasoning"));

        assert!(parts.text_response().is_none());
    }

    #[test]
    fn test_parts_structured_response() {
        let mut parts = Parts::default();
        let value = json!({"key": "value"});
        parts.push(PartEnum::from_structured(value.clone()));

        let structured = parts.structured_response().unwrap();
        assert_eq!(structured, &value);
    }

    #[test]
    fn test_parts_structured_response_none() {
        let mut parts = Parts::default();
        parts.push(PartEnum::from_text("Text only"));

        assert!(parts.structured_response().is_none());
    }

    #[test]
    fn test_parts_text_parts_iterator() {
        let mut parts = Parts::default();
        parts.push(PartEnum::from_text("First"));
        parts.push(PartEnum::from_reasoning("Thinking"));
        parts.push(PartEnum::from_text("Second"));

        let texts: Vec<&Text> = parts.text_parts().collect();
        assert_eq!(texts.len(), 2);
        assert_eq!(texts[0].as_str(), "First");
        assert_eq!(texts[1].as_str(), "Second");
    }

    #[test]
    fn test_parts_function_calls_iterator() {
        let mut parts = Parts::default();
        let fc1 = FunctionCall::new("func1".to_string(), json!({"arg": 1}));
        let fc2 = FunctionCall::new("func2".to_string(), json!({"arg": 2}));

        parts.push(PartEnum::from_function_call(fc1.clone()));
        parts.push(PartEnum::from_text("Text"));
        parts.push(PartEnum::from_function_call(fc2.clone()));

        let fcs: Vec<&FunctionCall> = parts.function_calls().collect();
        assert_eq!(fcs.len(), 2);
        assert_eq!(fcs[0].name, "func1");
        assert_eq!(fcs[1].name, "func2");
    }

    #[test]
    fn test_parts_function_responses_iterator() {
        let mut parts = Parts::default();
        let fc1 = FunctionCall::new("func1".to_string(), json!({}));
        let fr1 = FunctionResponse {
            id: fc1.id.clone(),
            name: "func1".to_string(),
            result: json!({"status": "ok"}),
        };

        parts.push(PartEnum::from_function_response(fr1.clone()));
        parts.push(PartEnum::from_text("Text"));

        let frs: Vec<&FunctionResponse> = parts.function_responses().collect();
        assert_eq!(frs.len(), 1);
        assert_eq!(frs[0].name, "func1");
    }

    #[test]
    fn test_parts_function_response_by_id() {
        let mut parts = Parts::default();
        let fc = FunctionCall::new("test_func".to_string(), json!({}));
        let fr = FunctionResponse {
            id: fc.id.clone(),
            name: "test_func".to_string(),
            result: json!({"result": 42}),
        };

        parts.push(PartEnum::from_function_response(fr.clone()));

        let found = parts.function_response(fc.id.clone().unwrap());
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "test_func");
    }

    #[test]
    fn test_parts_function_response_not_found() {
        let mut parts = Parts::default();
        let fc = FunctionCall::new("test".to_string(), json!({}));

        let found = parts.function_response(CallId::new());
        assert!(found.is_none());
    }

    #[test]
    fn test_part_enum_default() {
        let part = PartEnum::default();
        match part {
            PartEnum::Text(text) => assert_eq!(text.as_str(), ""),
            _ => panic!("Default should be empty text"),
        }
    }

    #[test]
    fn test_part_enum_from_text() {
        let part = PartEnum::from_text("Hello");
        assert_eq!(part.text().unwrap().as_str(), "Hello");
    }

    #[test]
    fn test_part_enum_from_reasoning() {
        let part = PartEnum::from_reasoning("Thinking...");
        assert_eq!(part.reasoning().unwrap().as_str(), "Thinking...");
    }

    #[test]
    fn test_part_enum_from_structured() {
        let value = json!({"data": [1, 2, 3]});
        let part = PartEnum::from_structured(value.clone());
        assert_eq!(part.structured().unwrap(), value);
    }

    #[test]
    fn test_part_enum_from_function_call() {
        let fc = FunctionCall::new("test".to_string(), json!({"arg": "value"}));
        let part = PartEnum::from_function_call(fc.clone());
        assert_eq!(part.function_call().unwrap().name, "test");
    }

    #[test]
    fn test_part_enum_from_function_response() {
        let fc = FunctionCall::new("test".to_string(), json!({}));
        let fr = FunctionResponse {
            id: fc.id.clone(),
            name: "test".to_string(),
            result: json!({"ok": true}),
        };
        let part = PartEnum::from_function_response(fr.clone());
        assert_eq!(part.function_response().unwrap().name, "test");
    }

    #[test]
    fn test_part_enum_text_getter() {
        let part = PartEnum::from_text("Test");
        assert!(part.text().is_some());
        assert!(part.reasoning().is_none());
        assert!(part.structured().is_none());
    }

    #[test]
    fn test_part_enum_reasoning_getter() {
        let part = PartEnum::from_reasoning("Think");
        assert!(part.reasoning().is_some());
        assert!(part.text().is_none());
    }

    #[test]
    fn test_part_enum_structured_getter() {
        let part = PartEnum::from_structured(json!({}));
        assert!(part.structured().is_some());
        assert!(part.text().is_none());
    }

    #[test]
    fn test_part_enum_function_call_getter() {
        let fc = FunctionCall::new("test".to_string(), json!({}));
        let part = PartEnum::from_function_call(fc);
        assert!(part.function_call().is_some());
        assert!(part.text().is_none());
    }

    #[test]
    fn test_part_enum_function_response_getter() {
        let fc = FunctionCall::new("test".to_string(), json!({}));
        let fr = FunctionResponse {
            id: fc.id,
            name: "test".to_string(),
            result: json!({}),
        };
        let part = PartEnum::from_function_response(fr);
        assert!(part.function_response().is_some());
        assert!(part.text().is_none());
    }

    #[test]
    fn test_part_enum_display_text() {
        let part = PartEnum::from_text("Display me");
        assert_eq!(format!("{}", part), "Display me");
    }

    #[test]
    fn test_part_enum_display_reasoning() {
        let part = PartEnum::from_reasoning("Reasoning text");
        assert_eq!(format!("{}", part), "Reasoning text");
    }

    #[test]
    fn test_part_enum_display_structured() {
        let part = PartEnum::from_structured(json!({"key": "value"}));
        let display = format!("{}", part);
        assert!(display.contains("key"));
        assert!(display.contains("value"));
    }

    #[test]
    fn test_part_enum_display_function_call() {
        let fc = FunctionCall::new("my_function".to_string(), json!({}));
        let part = PartEnum::from_function_call(fc);
        assert_eq!(format!("{}", part), "my_function");
    }

    #[test]
    fn test_part_enum_display_function_response() {
        let fc = FunctionCall::new("response_func".to_string(), json!({}));
        let fr = FunctionResponse {
            id: fc.id,
            name: "response_func".to_string(),
            result: json!({}),
        };
        let part = PartEnum::from_function_response(fr);
        assert_eq!(format!("{}", part), "response_func");
    }

    #[test]
    fn test_parts_serialization() {
        let mut parts = Parts::default();
        parts.push(PartEnum::from_text("Test"));

        let serialized = serde_json::to_string(&parts).unwrap();
        let deserialized: Parts = serde_json::from_str(&serialized).unwrap();
        assert_eq!(parts, deserialized);
    }

    #[test]
    fn test_part_enum_clone() {
        let part1 = PartEnum::from_text("Clone me");
        let part2 = part1.clone();
        assert_eq!(part1, part2);
    }

    #[test]
    fn test_parts_with_mixed_types() {
        let mut parts = Parts::default();
        parts.push(PartEnum::from_text("Text"));
        parts.push(PartEnum::from_reasoning("Reasoning"));
        parts.push(PartEnum::from_structured(json!({"key": "value"})));

        assert_eq!(parts.len(), 3);
        assert_eq!(parts.text_parts().count(), 1);
    }

    #[test]
    fn test_parts_empty_checks() {
        let parts = Parts::default();
        assert!(parts.is_empty());
        assert_eq!(parts.len(), 0);

        let mut parts2 = Parts::default();
        parts2.push(PartEnum::from_text("Not empty"));
        assert!(!parts2.is_empty());
        assert_eq!(parts2.len(), 1);
    }

    #[test]
    fn test_parts_equality() {
        let mut parts1 = Parts::default();
        parts1.push(PartEnum::from_text("Same"));

        let mut parts2 = Parts::default();
        parts2.push(PartEnum::from_text("Same"));

        assert_eq!(parts1, parts2);
    }

    #[test]
    fn test_part_enum_equality() {
        let part1 = PartEnum::from_text("Same");
        let part2 = PartEnum::from_text("Same");
        assert_eq!(part1, part2);

        let part3 = PartEnum::from_text("Different");
        assert_ne!(part1, part3);
    }
}
