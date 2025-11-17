use std::fmt::Display;

use serde::Serialize;
use tools_rs::{CallId, FunctionCall, FunctionResponse};

#[derive(Clone, Debug, Default)]
pub struct Messages(Vec<Content>);

#[derive(Clone, Debug, Default)]
pub struct Content {
    pub parts: Parts,
    pub role: RoleEnum,
}

#[derive(Default, Debug, Clone)]
enum RoleEnum {
    #[default]
    User,
    System,
    Model,
}

pub struct Prompt;
impl Prompt {
    fn user(prompts: Vec<&str>) -> Content {
        let role = RoleEnum::User;
        let parts = Parts(
            prompts
                .iter()
                .map(|prompt| PartEnum::from_text(prompt.to_string()))
                .collect(),
        );
        Content { role, parts }
    }

    fn system(prompts: Vec<&str>) -> Content {
        let role = RoleEnum::System;
        let parts = Parts(
            prompts
                .iter()
                .map(|prompt| PartEnum::from_text(prompt.to_string()))
                .collect(),
        );
        Content { role, parts }
    }

    fn model(prompts: Vec<String>) -> Content {
        let role = RoleEnum::Model;
        let parts = Parts(
            prompts
                .iter()
                .map(|prompt| PartEnum::from_text(prompt.to_string()))
                .collect(),
        );
        Content { role, parts }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Parts(Vec<PartEnum>);

impl Parts {
    fn text_response(&self) -> Option<&Text> {
        self.0
            .iter()
            .filter_map(|res| match res {
                PartEnum::Text(text) => Some(text),
                _ => None,
            })
            .next()
    }

    fn structured_response(&self) -> Option<&serde_json::Value> {
        self.0
            .iter()
            .filter_map(|res| match res {
                PartEnum::Structured(value) => Some(value),
                _ => None,
            })
            .next()
    }

    fn push(&mut self, part: PartEnum) -> &mut Self {
        self.0.push(part);
        self
    }

    fn text_parts(&self) -> impl Iterator<Item = &Text> + '_ {
        self.0.iter().filter_map(|p| match p {
            PartEnum::Text(text) => Some(text),
            _ => None,
        })
    }

    fn function_calls(&self) -> impl Iterator<Item = &FunctionCall> + '_ {
        self.0.iter().filter_map(|p| match p {
            PartEnum::FunctionCall(fc) => Some(fc),
            _ => None,
        })
    }

    fn function_response(&self) -> impl Iterator<Item = &FunctionResponse> + '_ {
        self.0.iter().filter_map(|p| match p {
            PartEnum::FunctionResponse(fr) => Some(fr),
            _ => None,
        })
    }

    fn function_status(&self, id: impl Into<CallId>) -> Option<&FunctionResponse> {
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

#[derive(Debug, Clone, Serialize)]
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

    pub fn from_function_response(fc: FunctionCall) -> PartEnum {
        PartEnum::FunctionCall(fc)
    }
    pub fn from_function_call(fr: FunctionResponse) -> PartEnum {
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

#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
pub struct Text(String);

impl Text {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<String> for Text {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl AsRef<str> for Text {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Display for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn main() {
    let prompt = Prompt::user(vec!["Hello there mom!"]);
    let sys_prompt = Prompt::system(vec![
        "Answer the user with clarity at all times. Be a good boy.",
    ]);
    let messages = vec![sys_prompt, prompt];
    println!("{:#?}", messages);
}
