use schemars::JsonSchema;
use serde::de::DeserializeOwned;
#[cfg(feature = "stream")]
use std::fmt;
#[cfg(feature = "stream")]
use tools_rs::{FunctionCall, FunctionResponse};

use crate::types::{
    messages::{content::Content, embeddings::Embeddings},
    metadata::Metadata,
};

#[derive(Clone, Debug)]
pub struct ChatResponse {
    pub metadata: Option<Metadata>,
    pub content: Content,
}

#[derive(Debug, Clone)]
pub struct StructuredResponse<T: DeserializeOwned + JsonSchema> {
    pub content: T,
    pub metadata: Option<Metadata>,
}

#[derive(Clone, Debug)]
pub struct EmbeddingsResponse {
    pub metadata: Option<Metadata>,
    pub embeddings: Embeddings,
}

#[cfg(feature = "stream")]
#[derive(Debug, Clone)]
pub enum StreamEvent {
    TextChunk(String),
    ReasoningChunk(String),
    ToolCall(FunctionCall),
    ToolResult(FunctionResponse),
    Done(ChatResponse),
}

#[cfg(feature = "stream")]
impl fmt::Display for StreamEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StreamEvent::TextChunk(text) => write!(f, "{}", text),
            StreamEvent::ReasoningChunk(text) => write!(f, "{}", text),
            StreamEvent::ToolCall(call) => {
                write!(f, "\n[System: Calling tool '{}'...]\n", call.name)
            }
            StreamEvent::ToolResult(res) => {
                writeln!(f, "[System: Tool '{}' executed successfully]\n", res.name)
            }
            StreamEvent::Done(_) => write!(f, ""),
        }
    }
}

#[cfg(feature = "stream")]
#[derive(Default)]
pub struct SseParser {
    buffer: String,
    current_data: String,
    current_event_type: Option<String>,
}

#[cfg(feature = "stream")]
impl SseParser {
    pub fn push(&mut self, chunk: &[u8]) {
        self.buffer.push_str(&String::from_utf8_lossy(chunk));
    }

    /// Returns the next SSE event as `(event_type, data)`.
    ///
    /// - `event_type` comes from the `event:` line (empty string if absent).
    /// - `data` is the concatenated `data:` line(s).
    ///
    /// An event is emitted when a `data:` line is followed by either a blank
    /// line or a new `event:` line, making the parser robust for streams that
    /// may not always send blank-line separators between events.
    pub fn next_event(&mut self) -> Option<(String, String)> {
        loop {
            let newline_pos = self.buffer.find('\n')?;
            let line = self.buffer[..newline_pos]
                .trim_end_matches('\r')
                .to_string();
            self.buffer = self.buffer[newline_pos + 1..].to_string();

            if line.is_empty() {
                if !self.current_data.is_empty() {
                    let data = self.current_data.trim().to_string();
                    self.current_data.clear();
                    let event_type = self.current_event_type.take().unwrap_or_default();
                    if data != "[DONE]" {
                        return Some((event_type, data));
                    }
                }
                continue;
            }

            if let Some(event_type) = line.strip_prefix("event: ") {
                // A new event: line while we still have buffered data means the
                // previous event ended without a blank line — flush it first.
                if !self.current_data.is_empty() {
                    let data = self.current_data.trim().to_string();
                    self.current_data.clear();
                    let prev_type = self.current_event_type.take().unwrap_or_default();
                    self.current_event_type = Some(event_type.to_string());
                    if data != "[DONE]" {
                        return Some((prev_type, data));
                    }
                } else {
                    self.current_event_type = Some(event_type.to_string());
                }
            } else if let Some(data) = line.strip_prefix("data: ") {
                self.current_data.push_str(data);
                self.current_data.push('\n');
            } else if let Some(data) = line.strip_prefix("data:") {
                self.current_data.push_str(data);
                self.current_data.push('\n');
            }
        }
    }
}
