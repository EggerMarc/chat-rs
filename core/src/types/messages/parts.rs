use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::slice::{Iter, IterMut};
use tools_rs::{CallId, FunctionCall, FunctionResponse};

use crate::types::messages::embeddings::Embeddings;
use crate::types::messages::file::{File, FileSource};
use crate::types::messages::reasoning::Reasoning;
use crate::types::messages::text::Text;
use crate::types::messages::tool::{Tool, ToolStatus};

#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
#[repr(transparent)]
pub struct Parts(pub Vec<PartEnum>);

impl IntoIterator for Parts {
    type Item = PartEnum;
    type IntoIter = std::vec::IntoIter<PartEnum>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Parts {
    type Item = &'a PartEnum;
    type IntoIter = Iter<'a, PartEnum>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

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

    /// Iterate over all Tool parts.
    pub fn tools(&self) -> impl Iterator<Item = &Tool> + '_ {
        self.0.iter().filter_map(|p| match p {
            PartEnum::Tool(t) => Some(t),
            _ => None,
        })
    }

    /// Mutable iterator over Tool parts. Used by the executor to flip
    /// statuses in place.
    pub fn tools_mut(&mut self) -> impl Iterator<Item = &mut Tool> + '_ {
        self.0.iter_mut().filter_map(|p| match p {
            PartEnum::Tool(t) => Some(t),
            _ => None,
        })
    }

    /// Tools awaiting human approval.
    pub fn pending_tools(&self) -> impl Iterator<Item = &Tool> + '_ {
        self.tools().filter(|t| t.is_pending())
    }

    /// Tools in a resolved state (Completed, Rejected, or Failed).
    pub fn resolved_tools(&self) -> impl Iterator<Item = &Tool> + '_ {
        self.tools().filter(|t| t.is_resolved())
    }

    /// Look up a Tool by its call id.
    pub fn tool(&self, id: &CallId) -> Option<&Tool> {
        self.tools().find(|t| &t.id == id)
    }

    /// Mutable lookup by id.
    pub fn tool_mut(&mut self, id: &CallId) -> Option<&mut Tool> {
        self.tools_mut().find(|t| &t.id == id)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum PartEnum {
    Reasoning(Reasoning),
    Text(Text),
    Tool(Tool),
    Structured(serde_json::Value),
    File(File),
    Embeddings(Embeddings),
}

impl Default for PartEnum {
    fn default() -> Self {
        PartEnum::Text(Text("".to_string()))
    }
}

impl PartEnum {
    /// Construct a `PartEnum::Tool` in the `Pending` state from a model-emitted call.
    pub fn from_function_call(fc: FunctionCall) -> PartEnum {
        PartEnum::Tool(Tool::new(fc))
    }

    /// Construct a `PartEnum::Tool` that is already `Completed` with the given response.
    ///
    /// Used by deserialization shims and tests. Runtime flows should instead
    /// construct a Pending tool and call [`Tool::complete`] after execution.
    pub fn from_function_response(fr: FunctionResponse) -> PartEnum {
        let id = fr.id.clone().unwrap_or_default();
        let call = FunctionCall {
            id: Some(id.clone()),
            name: fr.name.clone(),
            arguments: serde_json::Value::Null,
        };
        PartEnum::Tool(Tool {
            id,
            call,
            status: ToolStatus::Completed { response: fr },
        })
    }

    pub fn tool(&self) -> Option<&Tool> {
        match self {
            PartEnum::Tool(t) => Some(t),
            _ => None,
        }
    }

    pub fn function_call(&self) -> Option<FunctionCall> {
        match self {
            PartEnum::Tool(t) => Some(t.call.clone()),
            _ => None,
        }
    }

    pub fn function_response(&self) -> Option<FunctionResponse> {
        match self {
            PartEnum::Tool(t) => t.response().cloned(),
            _ => None,
        }
    }

    pub fn text(&self) -> Option<Text> {
        match self {
            PartEnum::Text(text) => Some(text.clone()),
            _ => None,
        }
    }

    pub fn reasoning(&self) -> Option<Reasoning> {
        match self {
            PartEnum::Reasoning(reasoning) => Some(reasoning.clone()),
            _ => None,
        }
    }

    pub fn embeddings(&self) -> Option<Embeddings> {
        match self {
            PartEnum::Embeddings(embeddings) => Some(embeddings.to_owned()),
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
        PartEnum::Reasoning(Reasoning::new(s))
    }

    pub fn from_text(s: impl Into<String>) -> PartEnum {
        PartEnum::Text(Text::new(s))
    }

    pub fn from_tool(tool: Tool) -> PartEnum {
        PartEnum::Tool(tool)
    }

    pub fn from_embeddings(embeddings: Embeddings) -> PartEnum {
        PartEnum::Embeddings(embeddings)
    }

    pub fn from_structured(value: serde_json::Value) -> PartEnum {
        PartEnum::Structured(value)
    }

    pub fn from_file(file: File) -> PartEnum {
        PartEnum::File(file)
    }
}

impl From<&str> for PartEnum {
    fn from(s: &str) -> Self {
        PartEnum::Text(Text::new(s))
    }
}

impl From<String> for PartEnum {
    fn from(s: String) -> Self {
        PartEnum::Text(Text(s))
    }
}

impl From<&String> for PartEnum {
    fn from(s: &String) -> Self {
        PartEnum::Text(Text(s.clone()))
    }
}

impl From<Text> for PartEnum {
    fn from(text: Text) -> Self {
        PartEnum::Text(text)
    }
}

impl From<Reasoning> for PartEnum {
    fn from(reasoning: Reasoning) -> Self {
        PartEnum::Reasoning(reasoning)
    }
}

impl From<Tool> for PartEnum {
    fn from(tool: Tool) -> Self {
        PartEnum::Tool(tool)
    }
}

impl From<File> for PartEnum {
    fn from(file: File) -> Self {
        PartEnum::File(file)
    }
}

impl From<Embeddings> for PartEnum {
    fn from(embeddings: Embeddings) -> Self {
        PartEnum::Embeddings(embeddings)
    }
}

impl From<serde_json::Value> for PartEnum {
    fn from(value: serde_json::Value) -> Self {
        PartEnum::Structured(value)
    }
}

/// Build a heterogeneous `Vec<PartEnum>` from any items that implement
/// `Into<PartEnum>`. Lets you mix text, files, tools, etc. in a single
/// expression without explicit per-item conversions.
///
/// ```ignore
/// use chat_core::parts;
/// let p = parts!["look at this", screenshot, "and tell me what you see"];
/// ```
#[macro_export]
macro_rules! parts {
    () => { ::std::vec::Vec::<$crate::types::messages::parts::PartEnum>::new() };
    ($($item:expr),+ $(,)?) => {
        ::std::vec![
            $(<$crate::types::messages::parts::PartEnum as ::core::convert::From<_>>::from($item)),+
        ]
    };
}

impl Display for PartEnum {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PartEnum::Structured(value) => write!(f, "{}", value),
            PartEnum::Reasoning(text) => write!(f, "{}", text),
            PartEnum::Text(text) => write!(f, "{}", text),
            PartEnum::Tool(tool) => write!(f, "{}", tool.call.name),
            PartEnum::File(file) => match &file.source {
                FileSource::Url(url) => write!(f, "{} {}", file.mime, url),
                FileSource::Bytes(bytes) => write!(f, "{} ({} bytes)", file.mime, bytes.len()),
            },
            PartEnum::Embeddings(embeddings) => {
                let content = &embeddings.content;
                let len = content.len();

                match len {
                    0 => write!(f, "[]"),
                    1..=3 => write!(f, "{:?}", content),
                    _ => write!(
                        f,
                        "[{:.4}, {:.4}, ..., {:.4}]",
                        content[0],
                        content[1],
                        content[len - 1]
                    ),
                }
            }
        }
    }
}

#[cfg(feature = "stream")]
impl Parts {
    pub fn merge_chunk(&mut self, part: PartEnum) -> Option<crate::types::response::StreamEvent> {
        use crate::types::response::StreamEvent;

        match part {
            PartEnum::Reasoning(new_r) => {
                let event = StreamEvent::ReasoningChunk(new_r.text.clone());
                if let Some(PartEnum::Reasoning(last_r)) = self.0.last_mut() {
                    last_r.text.push_str(&new_r.text);
                    if last_r.signature.is_none() && new_r.signature.is_some() {
                        last_r.signature = new_r.signature;
                    }
                } else {
                    self.push(PartEnum::Reasoning(new_r));
                }
                Some(event)
            }
            PartEnum::Text(new_t) => {
                let event = StreamEvent::TextChunk(new_t.0.clone());
                if let Some(PartEnum::Text(last_t)) = self.0.last_mut() {
                    last_t.0.push_str(&new_t.0);
                } else {
                    self.push(PartEnum::Text(new_t));
                }
                Some(event)
            }
            PartEnum::Tool(new_tool) => {
                if let Some(PartEnum::Tool(last)) = self.0.last_mut()
                    && !last.is_resolved()
                {
                    let last_fc = &mut last.call;
                    let new_fc = &new_tool.call;

                    let is_continuation = new_fc.name.is_empty() && new_fc.id.is_none();
                    let same_call = is_continuation
                        || (last_fc.name == new_fc.name
                            && match (&last_fc.id, &new_fc.id) {
                                (Some(last_id), Some(new_id)) => last_id == new_id,
                                _ => true,
                            });

                    if same_call {
                        match (&mut last_fc.arguments, &new_fc.arguments) {
                            (
                                serde_json::Value::String(existing),
                                serde_json::Value::String(new_str),
                            ) => {
                                existing.push_str(new_str);
                            }
                            _ => {
                                last_fc.arguments = new_fc.arguments.clone();
                            }
                        }
                        if last_fc.id.is_none() && new_fc.id.is_some() {
                            last_fc.id = new_fc.id.clone();
                            last.id = new_fc.id.clone().unwrap_or_else(CallId::new);
                        }
                        return None;
                    }
                }

                let emitted = StreamEvent::ToolCall(new_tool.call.clone());
                self.push(PartEnum::Tool(new_tool));
                Some(emitted)
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod conversion_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn from_str_and_string_produce_text_part() {
        assert!(matches!(PartEnum::from("hi"), PartEnum::Text(t) if t.as_str() == "hi"));
        assert!(
            matches!(PartEnum::from(String::from("hi")), PartEnum::Text(t) if t.as_str() == "hi")
        );
        let owned = String::from("hi");
        assert!(matches!(PartEnum::from(&owned), PartEnum::Text(t) if t.as_str() == "hi"));
    }

    #[test]
    fn from_file_and_value_produce_matching_variants() {
        let file = File::from_bytes_with_mime(b"x".to_vec(), "image/png");
        assert!(matches!(PartEnum::from(file), PartEnum::File(_)));
        assert!(matches!(
            PartEnum::from(json!({"k": 1})),
            PartEnum::Structured(_)
        ));
    }

    #[test]
    fn parts_macro_builds_heterogeneous_vec() {
        let file = File::from_bytes_with_mime(b"x".to_vec(), "image/png");
        let v = crate::parts!["look", file, String::from("then this")];
        assert_eq!(v.len(), 3);
        assert!(matches!(v[0], PartEnum::Text(_)));
        assert!(matches!(v[1], PartEnum::File(_)));
        assert!(matches!(v[2], PartEnum::Text(_)));
    }

    #[test]
    fn parts_macro_empty_yields_empty_vec() {
        let v: Vec<PartEnum> = crate::parts![];
        assert!(v.is_empty());
    }
}
