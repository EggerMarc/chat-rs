//! Human-in-the-loop tool execution with Claude.
//!
//! Mirrors `examples/gemini/hitl.rs` but routes through Anthropic's
//! Claude via `ClaudeBuilder`. The HITL surface — metadata attributes,
//! strategy closures, `ChatOutcome::Paused`, `chat.resume()` — is
//! provider-agnostic; the only difference from the Gemini example is
//! the client builder.

use std::io::{self, BufRead, Write};

use chat_rs::{
    Action, ChatBuilder, ChatOutcome, Messages, PauseReason, ScopedCollection, ToolStatus,
    claude::ClaudeBuilder, types::messages::content,
};
use serde::Deserialize;
use tools_rs::{FunctionCall, ToolCollection, tool};

// ── 1. App-defined metadata schema ────────────────────────────────────────

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
#[serde(rename_all = "snake_case")]
enum Safety {
    #[default]
    Safe,
    SideEffect,
    Destructive,
}

// ── 2. Tools declare attributes at the site ───────────────────────────────

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
            panic!("scheduled / mixed pauses are not handled in this example");
        }
        _ => panic!("unexpected pause reason"),
    }
}

// ── 5. Main ───────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let raw_tools: ToolCollection<ApprovalMeta> = ToolCollection::collect_tools()?;
    let scoped = ScopedCollection::new(raw_tools, approval_strategy);

    let client = ClaudeBuilder::new()
        .with_model("claude-sonnet-4-20250514".to_string())
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
