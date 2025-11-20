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
