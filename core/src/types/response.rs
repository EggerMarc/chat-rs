use schemars::JsonSchema;
use serde::de::DeserializeOwned;
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
    current_event: String,
}

#[cfg(feature = "stream")]
impl SseParser {
    pub fn push(&mut self, chunk: &[u8]) {
        self.buffer.push_str(&String::from_utf8_lossy(chunk));
    }

    pub fn next_event(&mut self) -> Option<String> {
        while let Some(newline_pos) = self.buffer.find('\n') {
            let line = self.buffer[..newline_pos].trim_end().to_string();
            self.buffer.drain(..newline_pos + 1);

            if line.is_empty() {
                if !self.current_event.is_empty() {
                    let json = self.current_event.trim().to_string();
                    self.current_event.clear();
                    if json != "[DONE]" {
                        return Some(json);
                    }
                }
                continue;
            }

            if let Some(data) = line.strip_prefix("data: ") {
                self.current_event.push_str(data);
                self.current_event.push('\n');
            } else if let Some(data) = line.strip_prefix("data:") {
                self.current_event.push_str(data);
                self.current_event.push('\n');
            }
        }
        None
    }
}
