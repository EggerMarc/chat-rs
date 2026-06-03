use std::collections::HashMap;

use serde_json::Value;
use tools_rs::ToolError;

use crate::{
    chat::state::Unstructured,
    error::ChatError,
    types::{
        callback::{CallbackStrategy, RetryStrategy},
        messages::{content::Content, tool::ToolStatus},
        options::ChatOptions,
        response::PauseReason,
        tools::{Action, ToolDeclarations, TypedCollection},
    },
};

pub mod completion;
pub mod embed;
pub mod state;
#[cfg(feature = "stream")]
pub mod stream;

#[derive(Default)]
pub struct Chat<CP, Output = Unstructured> {
    pub(crate) model: CP,
    pub(crate) output_shape: Option<schemars::Schema>,
    pub(crate) model_options: Option<ChatOptions>,
    pub(crate) max_steps: Option<u16>,
    pub(crate) max_retries: Option<u16>,
    pub(crate) retry_strategy: Option<RetryStrategy>,
    pub(crate) before_strategy: Option<CallbackStrategy>,
    pub(crate) after_strategy: Option<CallbackStrategy>,
    pub(crate) scoped_collections: Vec<Box<dyn TypedCollection>>,
    pub(crate) routing: HashMap<String, usize>,
    pub(crate) _output: std::marker::PhantomData<Output>,
}

/// Outcome of a single `tool_call` pass over the most recent Content.
///
/// - `executed`: at least one tool was run and populated (Completed/Failed/
///   Rejected). Drives the continue-vs-stop decision in the chat loop.
/// - `pause`: the strategy put at least one tool into a state that
///   requires the caller to act. If `Some`, the loop returns
///   `ChatOutcome::Paused` with this reason.
#[derive(Debug, Default)]
pub(crate) struct ToolCallPass {
    pub executed: bool,
    pub pause: Option<PauseReason>,
}

/// Aggregates every scoped collection's declarations into a single
/// `ToolDeclarations` implementation that providers consume.
///
/// Borrowed against `&Chat` — lives only for the duration of a single
/// request. Concatenation is lazy: `.json()` walks the collections on
/// demand, so there's no per-request allocation beyond the resulting
/// JSON array.
pub(crate) struct AggregatedDeclarations<'a> {
    collections: &'a [Box<dyn TypedCollection>],
}

impl<'a> ToolDeclarations for AggregatedDeclarations<'a> {
    fn json(&self) -> Result<Value, ToolError> {
        let mut all = Vec::new();
        for coll in self.collections {
            if let Value::Array(arr) = coll.declarations()? {
                all.extend(arr);
            }
        }
        Ok(Value::Array(all))
    }
}

/// Build an `AggregatedDeclarations` view directly from a slice. Used
/// by the chat loop so the borrow is scoped to `scoped_collections` and
/// not the whole `Chat` — lets callers still borrow `self.model`
/// mutably.
pub(crate) fn tool_declarations_from(
    collections: &[Box<dyn TypedCollection>],
) -> Option<AggregatedDeclarations<'_>> {
    if collections.is_empty() {
        None
    } else {
        Some(AggregatedDeclarations { collections })
    }
}

impl<P, Output> Chat<P, Output> {
    /// Look up which scoped collection owns a given tool name.
    fn collection_for(&self, name: &str) -> Option<&dyn TypedCollection> {
        self.routing
            .get(name)
            .and_then(|&idx| self.scoped_collections.get(idx).map(|b| b.as_ref()))
    }

    /// Run one pass over the tools in `content`: consult each tool's
    /// strategy, execute the ones that say `Execute`, leave the ones
    /// that require human approval or deferral in a non-resolved state
    /// and accumulate them into a `PauseReason`.
    pub(crate) async fn tool_call(&self, content: &mut Content) -> Result<ToolCallPass, ChatError> {
        let mut pass = ToolCallPass::default();

        let mut idx = 0;
        while idx < content.parts.0.len() {
            let part = &mut content.parts.0[idx];
            let tool = match part {
                crate::types::messages::parts::PartEnum::Tool(t) => t,
                _ => {
                    idx += 1;
                    continue;
                }
            };

            if tool.is_resolved() {
                idx += 1;
                continue;
            }

            let already_approved = matches!(tool.status, ToolStatus::Approved { .. });

            let action = if already_approved {
                Action::Execute
            } else {
                let coll = self.collection_for(&tool.call.name).ok_or_else(|| {
                    ChatError::InvalidResponse(format!(
                        "no scoped collection owns tool `{}`",
                        tool.call.name
                    ))
                })?;
                coll.decide(&tool.call)
            };

            match action {
                Action::Execute => {
                    let coll = self.collection_for(&tool.call.name).ok_or_else(|| {
                        ChatError::InvalidResponse(format!(
                            "no scoped collection owns tool `{}`",
                            tool.call.name
                        ))
                    })?;
                    tool.mark_running();
                    let call = tool.effective_call().clone();
                    match coll.call(call).await {
                        Ok(response) => tool.complete(response),
                        Err(ToolError::Runtime(msg)) => tool.fail(msg),
                        Err(e) => tool.fail(format!("{e:?}")),
                    }
                    pass.executed = true;
                }
                Action::RequireApproval => match &mut pass.pause {
                    None => {
                        pass.pause = Some(PauseReason::AwaitingApproval {
                            tool_ids: vec![tool.id.clone()],
                        });
                    }
                    Some(PauseReason::AwaitingApproval { tool_ids }) => {
                        tool_ids.push(tool.id.clone());
                    }
                    Some(PauseReason::Scheduled { .. }) => {
                        let prev = pass
                            .pause
                            .replace(PauseReason::AwaitingApproval { tool_ids: vec![] });
                        if let Some(PauseReason::Scheduled {
                            tool_ids: sch_ids,
                            earliest,
                        }) = prev
                        {
                            pass.pause = Some(PauseReason::Mixed {
                                approvals: vec![tool.id.clone()],
                                scheduled: sch_ids.into_iter().map(|id| (id, earliest)).collect(),
                            });
                        }
                    }
                    Some(PauseReason::Mixed { approvals, .. }) => {
                        approvals.push(tool.id.clone());
                    }
                },
                Action::Defer { at } => match &mut pass.pause {
                    None => {
                        pass.pause = Some(PauseReason::Scheduled {
                            tool_ids: vec![tool.id.clone()],
                            earliest: at,
                        });
                    }
                    Some(PauseReason::Scheduled { tool_ids, earliest }) => {
                        tool_ids.push(tool.id.clone());
                        if at < *earliest {
                            *earliest = at;
                        }
                    }
                    Some(PauseReason::AwaitingApproval { tool_ids }) => {
                        let approvals = std::mem::take(tool_ids);
                        pass.pause = Some(PauseReason::Mixed {
                            approvals,
                            scheduled: vec![(tool.id.clone(), at)],
                        });
                    }
                    Some(PauseReason::Mixed { scheduled, .. }) => {
                        scheduled.push((tool.id.clone(), at));
                    }
                },
                Action::Reject { reason } => {
                    tool.reject(Some(reason));
                    pass.executed = true;
                }
            }

            idx += 1;
        }

        Ok(pass)
    }
}
