use serde::{Deserialize, Serialize};
use serde_json::json;
use tools_rs::{CallId, FunctionCall, FunctionResponse};

/// A tool invocation paired with its lifecycle state.
///
/// Replaces the previous split of `PartEnum::FunctionCall` and
/// `PartEnum::FunctionResponse`. Call and result live on the same part; the
/// `status` field tracks where the tool is in its lifecycle (waiting for
/// human approval, running, completed, rejected, or failed).
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct Tool {
    pub id: CallId,
    pub call: FunctionCall,
    pub status: ToolStatus,
}

/// Lifecycle of a tool call.
///
/// `Pending`, `Approved`, and `Running` are local-only states — they never
/// appear on the wire. Providers only see resolved tools (`Completed`,
/// `Rejected`, `Failed`) via [`Tool::to_tuple`].
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(tag = "state", content = "data")]
pub enum ToolStatus {
    Pending,
    Approved {
        #[serde(skip_serializing_if = "Option::is_none")]
        edited_call: Option<FunctionCall>,
    },
    Running,
    Completed {
        response: FunctionResponse,
    },
    Rejected {
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    Failed {
        error: String,
    },
}

impl ToolStatus {
    pub fn state_name(&self) -> &'static str {
        match self {
            ToolStatus::Pending => "Pending",
            ToolStatus::Approved { .. } => "Approved",
            ToolStatus::Running => "Running",
            ToolStatus::Completed { .. } => "Completed",
            ToolStatus::Rejected { .. } => "Rejected",
            ToolStatus::Failed { .. } => "Failed",
        }
    }
}

impl Tool {
    /// Create a new Tool in `Pending` state from a model-emitted call.
    /// Uses the call's existing id, or mints a fresh one.
    pub fn new(call: FunctionCall) -> Self {
        let id = call.id.clone().unwrap_or_default();
        Self {
            id,
            call,
            status: ToolStatus::Pending,
        }
    }

    /// True if the Tool has reached a final state (Completed, Rejected, Failed).
    /// Resolved tools are safe to send on the wire.
    pub fn is_resolved(&self) -> bool {
        matches!(
            self.status,
            ToolStatus::Completed { .. } | ToolStatus::Rejected { .. } | ToolStatus::Failed { .. }
        )
    }

    pub fn is_pending(&self) -> bool {
        matches!(self.status, ToolStatus::Pending)
    }

    /// True if the tool has been approved (or auto-approved) and is either
    /// queued or running.
    pub fn is_executing(&self) -> bool {
        matches!(
            self.status,
            ToolStatus::Approved { .. } | ToolStatus::Running
        )
    }

    /// The call as it should actually be executed. Honors edits made during
    /// approval; otherwise returns the original model-emitted call.
    pub fn effective_call(&self) -> &FunctionCall {
        match &self.status {
            ToolStatus::Approved {
                edited_call: Some(c),
            } => c,
            _ => &self.call,
        }
    }

    /// The successful response, if any. Returns `None` for any non-Completed
    /// state — including Failed and Rejected. Use [`Tool::to_tuple`] for the
    /// wire-ready projection.
    pub fn response(&self) -> Option<&FunctionResponse> {
        match &self.status {
            ToolStatus::Completed { response } => Some(response),
            _ => None,
        }
    }

    pub fn approve(&mut self, edited_call: Option<FunctionCall>) {
        self.status = ToolStatus::Approved { edited_call };
    }

    pub fn reject(&mut self, reason: Option<String>) {
        self.status = ToolStatus::Rejected { reason };
    }

    pub fn mark_running(&mut self) {
        self.status = ToolStatus::Running;
    }

    pub fn complete(&mut self, response: FunctionResponse) {
        self.status = ToolStatus::Completed { response };
    }

    pub fn fail(&mut self, error: impl Into<String>) {
        self.status = ToolStatus::Failed {
            error: error.into(),
        };
    }

    /// Project the Tool into the `(call, optional response)` pair that
    /// providers put on the wire.
    ///
    /// - `Completed` → real response
    /// - `Failed`    → synthesized error response carrying the error string
    /// - `Rejected`  → synthesized error response noting the refusal
    /// - `Pending` / `Approved` / `Running` → `(call, None)` — not ready
    ///
    /// This is the only place in the codebase where lossy ToolStatus → wire
    /// conversion happens. Providers must not re-implement it.
    pub fn to_tuple(&self) -> (FunctionCall, Option<FunctionResponse>) {
        let call = self.effective_call().clone();
        let response = match &self.status {
            ToolStatus::Completed { response } => Some(response.clone()),
            ToolStatus::Failed { error } => {
                Some(synth_error(&self.id, &self.call.name, error.clone()))
            }
            ToolStatus::Rejected { reason } => Some(synth_error(
                &self.id,
                &self.call.name,
                format!(
                    "User rejected this tool call{}",
                    reason
                        .as_ref()
                        .map(|r| format!(": {r}"))
                        .unwrap_or_default()
                ),
            )),
            ToolStatus::Pending | ToolStatus::Approved { .. } | ToolStatus::Running => None,
        };
        (call, response)
    }

    /// Strict variant: errors if the Tool is not resolved. Use in provider
    /// request builders where sending a non-resolved Tool would be a bug.
    pub fn try_to_tuple(&self) -> Result<(FunctionCall, FunctionResponse), NonResolvedToolError> {
        match self.to_tuple() {
            (call, Some(response)) => Ok((call, response)),
            (_, None) => Err(NonResolvedToolError {
                id: self.id.clone(),
                state: self.status.state_name(),
            }),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("cannot send non-resolved Tool (id={id}, state={state})")]
pub struct NonResolvedToolError {
    pub id: CallId,
    pub state: &'static str,
}

fn synth_error(id: &CallId, name: &str, message: String) -> FunctionResponse {
    FunctionResponse {
        id: Some(id.clone()),
        name: name.to_string(),
        result: json!({ "error": message }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn fc(name: &str) -> FunctionCall {
        FunctionCall {
            id: Some(CallId::new()),
            name: name.to_string(),
            arguments: json!({}),
        }
    }

    #[test]
    fn new_starts_pending() {
        let t = Tool::new(fc("ls"));
        assert!(t.is_pending());
        assert!(!t.is_resolved());
        assert!(!t.is_executing());
    }

    #[test]
    fn lifecycle_transitions() {
        let mut t = Tool::new(fc("ls"));
        t.approve(None);
        assert!(t.is_executing());
        t.mark_running();
        assert!(t.is_executing());
        t.complete(FunctionResponse {
            id: Some(t.id.clone()),
            name: "ls".to_string(),
            result: json!("file1"),
        });
        assert!(t.is_resolved());
        assert!(t.response().is_some());
    }

    #[test]
    fn rejected_to_tuple_synthesizes_error() {
        let mut t = Tool::new(fc("rm"));
        t.reject(Some("too dangerous".to_string()));
        let (_, resp) = t.to_tuple();
        let resp = resp.expect("rejected should produce a synthesized response");
        assert_eq!(
            resp.result["error"].as_str().unwrap(),
            "User rejected this tool call: too dangerous"
        );
    }

    #[test]
    fn failed_to_tuple_synthesizes_error() {
        let mut t = Tool::new(fc("net"));
        t.fail("timeout");
        let (_, resp) = t.to_tuple();
        let resp = resp.unwrap();
        assert_eq!(resp.result["error"].as_str().unwrap(), "timeout");
    }

    #[test]
    fn pending_to_tuple_yields_no_response() {
        let t = Tool::new(fc("ls"));
        assert!(t.to_tuple().1.is_none());
    }

    #[test]
    fn try_to_tuple_errors_on_pending() {
        let t = Tool::new(fc("ls"));
        assert!(t.try_to_tuple().is_err());
    }

    #[test]
    fn approved_with_edit_uses_edited_call() {
        let mut t = Tool::new(fc("ls"));
        let edited = FunctionCall {
            id: t.call.id.clone(),
            name: "ls".to_string(),
            arguments: json!({"path": "/tmp"}),
        };
        t.approve(Some(edited));
        let (call, _) = t.to_tuple();
        assert_eq!(call.arguments, json!({"path": "/tmp"}));
    }

    #[test]
    fn serde_round_trip() {
        let mut t = Tool::new(fc("ls"));
        t.complete(FunctionResponse {
            id: Some(t.id.clone()),
            name: "ls".to_string(),
            result: Value::String("ok".to_string()),
        });
        let s = serde_json::to_string(&t).unwrap();
        let back: Tool = serde_json::from_str(&s).unwrap();
        assert_eq!(t, back);
    }
}
