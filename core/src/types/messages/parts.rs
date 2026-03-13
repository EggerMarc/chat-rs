use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::slice::{Iter, IterMut};
use tools_rs::{CallId, FunctionCall, FunctionResponse};

use crate::types::messages::embeddings::Embeddings;
use crate::types::messages::file::File;
use crate::types::messages::reasoning::Reasoning;
use crate::types::messages::text::Text;

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
    Reasoning(Reasoning),
    Text(Text),
    FunctionCall(FunctionCall),
    FunctionResponse(FunctionResponse),
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

    /// Retrieve the reasoning text if this part is the `Reasoning` variant.
    ///
    /// # Returns
    ///
    /// `Some(Text)` containing the reasoning text if this is a `Reasoning` variant, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// let part = PartEnum::from_reasoning("because it's correct");
    /// let text = part.reasoning().unwrap();
    /// assert_eq!(text, Text::new("because it's correct"));
    /// ```
    pub fn reasoning(&self) -> Option<Reasoning> {
        match self {
            PartEnum::Reasoning(reasoning) => Some(reasoning.clone()),
            _ => None,
        }
    }

    /// Get the contained embeddings when the variant is `Embeddings`.
    ///
    /// # Returns
    ///
    /// `Some(Embeddings)` with a cloned value when the part is `PartEnum::Embeddings`, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::messages::parts::PartEnum;
    /// use crate::messages::embeddings::Embeddings;
    ///
    /// let emb = Embeddings { content: vec![0.1_f32, 0.2, 0.3] };
    /// let part = PartEnum::from_embeddings(emb.clone());
    /// assert_eq!(part.embeddings(), Some(emb));
    /// ```
    pub fn embeddings(&self) -> Option<Embeddings> {
        match self {
            PartEnum::Embeddings(embeddings) => Some(embeddings.to_owned()),
            _ => None,
        }
    }

    /// Returns the contained JSON value if this part is a `Structured` variant.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_json::json;
    /// use crate::core::messages::parts::PartEnum;
    ///
    /// let part = PartEnum::Structured(json!({"key": "value"}));
    /// assert_eq!(part.structured(), Some(json!({"key": "value"})));
    /// ```
    pub fn structured(&self) -> Option<serde_json::Value> {
        match self {
            PartEnum::Structured(value) => Some(value.clone()),
            _ => None,
        }
    }

    /// Accesses the contained File when this enum is the `File` variant.
    ///
    /// # Returns
    ///
    /// `Some(File)` containing the file when the variant is `PartEnum::File`, `None` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// use crate::core::messages::parts::PartEnum;
    /// use crate::messages::file::File;
    ///
    /// let file = File::default();
    /// let part = PartEnum::from_file(file.clone());
    /// assert_eq!(part.file(), Some(file));
    /// ```
    pub fn file(&self) -> Option<File> {
        match self {
            PartEnum::File(file) => Some(file.clone()),
            _ => None,
        }
    }

    /// Creates a `PartEnum::Reasoning` from the provided string.
    ///
    /// # Examples
    ///
    /// ```
    /// let part = PartEnum::from_reasoning("inference detail");
    /// match part {
    ///     PartEnum::Reasoning(text) => assert_eq!(text.as_str(), "inference detail"),
    ///     _ => panic!("expected Reasoning variant"),
    /// }
    /// ```
    pub fn from_reasoning(s: impl Into<String>) -> PartEnum {
        PartEnum::Reasoning(Reasoning::new(s))
    }

    pub fn from_text(s: impl Into<String>) -> PartEnum {
        PartEnum::Text(Text::new(s))
    }

    /// Creates a PartEnum containing the given FunctionCall.
    ///
    /// Returns the `PartEnum::FunctionCall` variant wrapping `fc`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let fc = FunctionCall::default(); // construct a FunctionCall as appropriate
    /// let part = PartEnum::from_function_call(fc);
    /// matches!(part, PartEnum::FunctionCall(_));
    /// ```
    pub fn from_function_call(fc: FunctionCall) -> PartEnum {
        PartEnum::FunctionCall(fc)
    }

    /// Wraps a `FunctionResponse` in a `PartEnum::FunctionResponse`.
    ///
    /// # Parameters
    ///
    /// - `fr`: the `FunctionResponse` to wrap.
    ///
    /// # Returns
    ///
    /// A `PartEnum::FunctionResponse` containing the provided `FunctionResponse`.
    pub fn from_function_response(fr: FunctionResponse) -> PartEnum {
        PartEnum::FunctionResponse(fr)
    }

    /// Creates a `PartEnum` that wraps the provided `Embeddings`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let embeddings = /* obtain an Embeddings value */ unimplemented!();
    /// let part = from_embeddings(embeddings);
    /// match part {
    ///     PartEnum::Embeddings(e) => { /* use `e` */ }
    ///     _ => unreachable!(),
    /// }
    /// ```
    pub fn from_embeddings(embeddings: Embeddings) -> PartEnum {
        PartEnum::Embeddings(embeddings)
    }

    /// Creates a `PartEnum::Structured` variant containing the given JSON `value`.
    ///
    /// # Examples
    ///
    /// ```
    /// use serde_json::json;
    ///
    /// let value = json!({ "key": "value" });
    /// let part = PartEnum::from_structured(value.clone());
    ///
    /// match part {
    ///     PartEnum::Structured(v) => assert_eq!(v, value),
    ///     _ => panic!("expected Structured variant"),
    /// }
    /// ```
    pub fn from_structured(value: serde_json::Value) -> PartEnum {
        PartEnum::Structured(value)
    }

    /// Creates a `PartEnum::File` variant containing the provided `File`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let file: File = /* obtain or construct a File */ unimplemented!();
    /// let part = PartEnum::from_file(file);
    /// match part {
    ///     PartEnum::File(f) => { /* use f */ },
    ///     _ => unreachable!(),
    /// }
    /// ```
    pub fn from_file(file: File) -> PartEnum {
        PartEnum::File(file)
    }
}

impl Display for PartEnum {
    /// Render a `PartEnum` as a concise, human-readable string.
    ///
    /// Each variant is represented as:
    /// - `Structured`: the JSON value is printed.
    /// - `Reasoning` and `Text`: the contained text is printed.
    /// - `FunctionCall` and `FunctionResponse`: the function name is printed.
    /// - `File`: prints the URL and MIME type for `File::Url`, or the MIME type for `File::Bytes`.
    /// - `Embeddings`: if the embedding vector is empty prints `[]`; if it has 1–3 elements prints the debug
    ///   representation of the full vector; if it has more than 3 elements prints a truncated preview
    ///   formatted as `[first, second, ..., last]` with 4-decimal precision for the shown values.
    ///
    /// # Examples
    ///
    /// ```
    /// let p = PartEnum::from_text("hello");
    /// assert_eq!(format!("{}", p), "hello");
    /// ```
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
            PartEnum::FunctionCall(new_fc) => {
                if let Some(PartEnum::FunctionCall(last_fc)) = self.0.last_mut() {
                    let same_call = last_fc.name == new_fc.name
                        && match (&last_fc.id, &new_fc.id) {
                            (Some(last_id), Some(new_id)) => last_id == new_id,
                            _ => true,
                        };
                    if same_call {
                        last_fc.arguments = new_fc.arguments.clone();
                        if last_fc.id.is_none() && new_fc.id.is_some() {
                            last_fc.id = new_fc.id.clone();
                        }
                        None
                    } else {
                        self.push(PartEnum::FunctionCall(new_fc.clone()));
                        Some(StreamEvent::ToolCall(new_fc))
                    }
                } else {
                    self.push(PartEnum::FunctionCall(new_fc.clone()));
                    Some(StreamEvent::ToolCall(new_fc))
                }
            }
            _ => None,
        }
    }
}
