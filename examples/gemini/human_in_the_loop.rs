//! Human-in-the-loop tool execution with Gemini.
//!
//! Demonstrates the HITL surface introduced in 0.0.15:
//!
//! - Tools declare their safety attributes at definition time via
//!   `#[tool(key = value, ...)]`, absorbed by a user-defined metadata
//!   struct at `collect_tools` time.
//! - A strategy closure translates metadata into `Action` — the chat-rs
//!   vocabulary for loop-flow decisions (execute / approve / defer / reject).
//! - `chat.complete()` pauses when the strategy says `RequireApproval`,
//!   handing control back via `ChatOutcome::Paused { reason }`.
//! - The caller resolves pending tools by mutating `ToolStatus` in place
//!   (via `Messages::find_tool_mut`), then calls `chat.resume()`.
//! - Metadata never leaks into tool execution — it only changes how the
//!   loop interacts with the tool.

use std::io::{self, BufRead, Write};

use chat_rs::{
    Action, ChatBuilder, ChatOutcome, Messages, PauseReason, ScopedCollection, ToolStatus,
    gemini::{self},
    types::messages::content,
};
use serde::Deserialize;
use tools_rs::{FunctionCall, ToolCollection, tool};

// ── 1. App-defined metadata schema ────────────────────────────────────────
//
// The app decides what attributes it cares about. `serde(default)` means
// tools that don't declare a field get the default. Unknown attributes
// declared on the tool (that this schema doesn't consume) are silently
// ignored.

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
struct ApprovalMeta {
    /// If true, the chat loop pauses and asks the user before running.
    requires_approval: bool,
    /// Freeform safety tier. Informs rendering and logging but isn't
    /// consulted by the strategy in this example.
    #[allow(dead_code)]
    safety: Safety,
}

#[derive(Debug, Default, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Safety {
    #[default]
    Safe,
    SideEffect,
    Destructive,
}

// ── 2. Tools declare attributes at the site ───────────────────────────────
//
// Each attribute value is absorbed into `ApprovalMeta` when the
// collection is built. Typos in keys surface at startup via
// `collect_tools` returning `ToolError::BadMeta`.

#[tool(requires_approval = true, safety = "destructive")]
/// Deletes files matching a glob pattern.
async fn delete_files(pattern: String) -> String {
    format!("(pretend) deleted files matching: {pattern}")
}

#[tool(requires_approval = true, safety = "side_effect")]
/// Sends an email. Side-effectful; requires human approval.
async fn send_email(to: String, subject: String) -> String {
    format!("(pretend) email sent to {to} — subject: {subject}")
}

#[tool(safety = "safe")]
/// Reads the contents of a file. Safe; no approval needed.
async fn read_file(path: String) -> String {
    format!("(pretend) contents of {path}: hello world")
}

// ── 3. Strategy closure ───────────────────────────────────────────────────
//
// Takes a call + metadata, returns an Action. This is where the app
// translates metadata into loop-flow decisions. The same metadata
// schema could pair with different strategies per deployment (dev
// auto-approves, prod gates, staging gates selectively).

fn approval_strategy(_call: &FunctionCall, meta: &ApprovalMeta) -> Action {
    if meta.requires_approval {
        Action::RequireApproval
    } else {
        Action::Execute
    }
}

// ── 4. User interaction ───────────────────────────────────────────────────

fn prompt(label: &str) -> io::Result<String> {
    print!("{label}");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

enum Decision {
    Approve,
    Reject(String),
}

fn ask_user(tool_name: &str, args: &serde_json::Value) -> Decision {
    println!("\n┌─ tool call pending human review ─");
    println!("│ name: {tool_name}");
    println!("│ args: {args}");
    println!("└───────────────────────────────────");

    loop {
        match prompt("approve? [y/n/reason]: ")
            .unwrap_or_default()
            .as_str()
        {
            "y" | "Y" | "yes" => return Decision::Approve,
            "n" | "N" | "no" => return Decision::Reject("denied by user".into()),
            other if !other.is_empty() => return Decision::Reject(other.into()),
            _ => println!("  (enter 'y', 'n', or a reason)"),
        }
    }
}

/// Walk the `Paused` reason's tool ids, prompting the user for each.
/// Mutates tool statuses on the `Messages` in place.
fn resolve_pending(reason: &PauseReason, messages: &mut Messages) {
    match reason {
        PauseReason::AwaitingApproval { tool_ids } => {
            for id in tool_ids {
                let Some(tool) = messages.find_tool_mut(id) else {
                    eprintln!("warning: paused tool id {id} not found in messages");
                    continue;
                };
                match ask_user(&tool.call.name, &tool.call.arguments) {
                    Decision::Approve => tool.approve(None),
                    Decision::Reject(reason) => tool.reject(Some(reason)),
                }
            }
        }
        PauseReason::Scheduled { .. } | PauseReason::Mixed { .. } => {
            // This example only produces RequireApproval pauses; a full
            // scheduler would persist and wake up at the given time.
            panic!("scheduled / mixed pauses are not handled in this example");
        }
        _ => panic!("unexpected pause reason"),
    }
}

// ── 5. Main ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build the typed collection. `collect_tools()` fails fast at startup
    // if any tool's declared attributes don't fit ApprovalMeta.
    let raw_tools: ToolCollection<ApprovalMeta> = ToolCollection::collect_tools()?;

    // Attach the strategy. From here on, the collection is type-erased
    // behind `dyn TypedCollection` — chat-rs doesn't see ApprovalMeta.
    let scoped = ScopedCollection::new(raw_tools, approval_strategy);

    let client = gemini::GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        .build();

    let mut chat = ChatBuilder::new()
        .with_model(client)
        .with_scoped_tools(scoped)
        .with_max_steps(8)
        .build();

    let mut messages = Messages::default();
    messages.push(content::from_system(vec![
        "You are a terse assistant with destructive tools available. \
         Use them when asked. Explain what you did afterwards.",
    ]));
    messages.push(content::from_user(vec![
        "Delete everything matching /tmp/*.log, then email ops@example.com \
         with subject 'cleanup done'. Also read /etc/hostname.",
    ]));

    // ── Pause/resume loop ────────────────────────────────────────────────
    //
    // `complete()` drives the model-tool loop internally. It returns when
    // the conversation is either finished or stuck on a pending decision.
    // On pause, we resolve pending tools in place and call `resume()`.

    let mut outcome = chat.complete(&mut messages).await?;
    loop {
        match outcome {
            ChatOutcome::Complete(response) => {
                println!("\nassistant: {:?}", response.content.parts.last());
                break;
            }
            ChatOutcome::Paused { reason } => {
                resolve_pending(&reason, &mut messages);
                outcome = chat.resume(&mut messages).await?;
            }
        }
    }

    // ── Final audit ──────────────────────────────────────────────────────
    //
    // Every tool call in the history carries its full lifecycle state on
    // a single `PartEnum::Tool` part. Persistence layers can serialize
    // this directly — one row per call, no pair reassembly.

    println!("\n--- tool audit ---");
    for content in &messages.0 {
        for t in content.parts.tools() {
            let state = match &t.status {
                ToolStatus::Completed { response } => {
                    format!("completed → {}", response.result)
                }
                ToolStatus::Rejected { reason } => {
                    format!("rejected ({})", reason.as_deref().unwrap_or("no reason"))
                }
                ToolStatus::Failed { error } => format!("failed: {error}"),
                other => other.state_name().to_string(),
            };
            println!("  {} [{}] · {state}", t.call.name, t.id);
        }
    }

    Ok(())
}
