use schemars::JsonSchema;
use serde::de::DeserializeOwned;
#[cfg(feature = "stream")]
use std::fmt;
use tools_rs::CallId;
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

/// Result of a chat loop iteration. Returned by `Chat::complete` and
/// `Chat::resume`.
///
/// The loop completes cleanly with `Complete` when the model has finished
/// its response without leaving tools in a non-resolved state. It returns
/// `Paused` when the loop stopped because one or more tools need the
/// caller to act — for example, a human has to approve a pending call or
/// a scheduled tool must wait for its wake-up time.
///
/// On `Paused`, the `Messages` argument that was passed in has been
/// mutated in place — tool statuses reflect where each call stands.
/// Callers resolve the pending tools by mutating their `ToolStatus`
/// (typically via `Messages::find_tool_mut`) and then calling
/// `Chat::resume` with the same `Messages`.
#[derive(Debug)]
pub enum ChatOutcome<T> {
    Complete(T),
    Paused { reason: PauseReason },
}

impl<T> ChatOutcome<T> {
    /// Convenience: extract the completed value, returning an error if
    /// the loop paused instead. Useful in flows that don't expect pauses.
    pub fn into_complete(self) -> Result<T, PauseReason> {
        match self {
            ChatOutcome::Complete(v) => Ok(v),
            ChatOutcome::Paused { reason } => Err(reason),
        }
    }

    /// Extract the completed value. Panics if the loop paused — use only
    /// in flows that don't configure any pausing strategies (no
    /// `Action::RequireApproval`, no `Action::Defer`).
    pub fn expect_complete(self) -> T {
        match self {
            ChatOutcome::Complete(v) => v,
            ChatOutcome::Paused { reason } => {
                panic!(
                    "expected chat loop to complete, but it paused: {reason:?}"
                )
            }
        }
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, ChatOutcome::Complete(_))
    }
}

/// Why the chat loop paused.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PauseReason {
    /// One or more tools are awaiting a human decision (approve, reject,
    /// or edit-then-approve). Caller is expected to resolve each tool in
    /// the list and call `Chat::resume`.
    AwaitingApproval { tool_ids: Vec<CallId> },

    /// One or more tools have been deferred to a future time. Caller
    /// should persist `Messages`, schedule a wake-up at `earliest`, and
    /// call `Chat::resume` when any deferred tool is ready.
    Scheduled {
        tool_ids: Vec<CallId>,
        earliest: std::time::SystemTime,
    },

    /// Mixed pause — some tools need approval, some are scheduled.
    Mixed {
        approvals: Vec<CallId>,
        scheduled: Vec<(CallId, std::time::SystemTime)>,
    },
}

impl PauseReason {
    /// Every tool id referenced by this pause, regardless of variant.
    /// Callers that just want "which tools do I need to look at" can
    /// use this instead of matching on the variant.
    pub fn tool_ids(&self) -> Vec<&CallId> {
        match self {
            PauseReason::AwaitingApproval { tool_ids }
            | PauseReason::Scheduled { tool_ids, .. } => tool_ids.iter().collect(),
            PauseReason::Mixed {
                approvals,
                scheduled,
            } => approvals
                .iter()
                .chain(scheduled.iter().map(|(id, _)| id))
                .collect(),
        }
    }
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
#[non_exhaustive]
pub enum StreamEvent {
    TextChunk(String),
    ReasoningChunk(String),
    ToolCall(FunctionCall),
    ToolResult(FunctionResponse),
    /// The chat loop paused because one or more tools need caller
    /// action (approval, scheduling, etc). This is the streaming
    /// equivalent of `ChatOutcome::Paused`. After receiving this event
    /// the stream terminates; the caller resolves pending tools on the
    /// `Messages` and calls `Chat::stream` again to continue.
    Paused(PauseReason),
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
            StreamEvent::Paused(_) => write!(f, "\n[System: Paused — caller action required]\n"),
            StreamEvent::Done(_) => write!(f, ""),
        }
    }
}

/// Re-export for backwards compatibility — `SseParser` now lives in
/// [`crate::transport::sse`].
pub use crate::transport::sse::SseParser;
