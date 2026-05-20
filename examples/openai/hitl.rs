//! Interactive HITL chat loop with OpenAI, streaming responses.
//!
//! A REPL: read a line from stdin, stream the assistant's response
//! token-by-token, pause when the tool strategy requires human approval,
//! and resume the stream after the user decides. Loops until Ctrl-D or
//! an empty line.
//!
//! Demonstrates:
//! - `#[tool(...)]` attributes absorbed by a typed metadata struct
//! - Strategy closure deciding `Action::Execute` vs `Action::RequireApproval`
//! - Streaming responses via `chat.stream()` yielding `StreamEvent`s
//! - Pause/resume on `StreamEvent::Paused(PauseReason)` — the stream
//!   terminates, caller resolves pending tools via
//!   `Messages::find_tool_mut`, then calls `chat.stream()` again to
//!   continue the same conversational turn
//! - Multi-turn history preserved across calls via a shared `Messages`

use std::io::{self, BufRead, Write};

use chat_rs::{
    Action, Chat, ChatBuilder, Messages, PauseReason, ScopedCollection, StreamEvent,
    StreamProvider, ToolStatus, Unstructured, openai::OpenAIBuilder, parts,
    types::messages::content,
};
use futures::StreamExt;
use serde::Deserialize;
use tools_rs::{FunctionCall, ToolCollection, tool};

// ── 1. App-defined metadata schema ────────────────────────────────────────

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
struct ApprovalMeta {
    requires_approval: bool,
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

// ── 2. Tools ──────────────────────────────────────────────────────────────

#[tool(requires_approval = true, safety = "destructive")]
/// Deletes files matching a glob pattern.
async fn delete_files(pattern: String) -> String {
    format!("(pretend) deleted files matching: {pattern}")
}

#[tool(requires_approval = true, safety = "side_effect")]
/// Sends an email.
async fn send_email(to: String, subject: String) -> String {
    format!("(pretend) email sent to {to} — subject: {subject}")
}

#[tool(safety = "safe")]
/// Reads the contents of a file.
async fn read_file(path: String) -> String {
    format!("(pretend) contents of {path}: hello world")
}

// ── 3. Strategy ───────────────────────────────────────────────────────────

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
    let bytes = io::stdin().lock().read_line(&mut line)?;
    if bytes == 0 {
        // EOF
        return Ok(String::new());
    }
    Ok(line.trim_end_matches('\n').to_string())
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
        match prompt("  approve? [y/n/reason]: ")
            .unwrap_or_default()
            .as_str()
        {
            "y" | "Y" | "yes" => return Decision::Approve,
            "n" | "N" | "no" => return Decision::Reject("denied by user".into()),
            "" => return Decision::Reject("denied by user".into()),
            other => return Decision::Reject(other.into()),
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

// ── 5. Stream one turn, handling pauses in-line ───────────────────────────
//
// Drives `chat.stream()` to exhaustion. If the stream yields a `Paused`
// event, resolves the pending tools in place and re-enters the stream
// by calling `chat.stream()` again on the same mutated `Messages`. The
// core loop's pre-step picks up the newly-approved tools and continues
// from where the previous stream left off.

async fn run_turn<CP: StreamProvider>(
    chat: &mut Chat<CP, Unstructured>,
    messages: &mut Messages,
) -> Result<(), Box<dyn std::error::Error>> {
    print!("assistant: ");
    io::stdout().flush()?;

    loop {
        let mut paused_reason: Option<PauseReason> = None;
        {
            let mut stream = chat.stream(messages).await?;
            while let Some(evt) = stream.next().await {
                let evt = evt?;
                match evt {
                    StreamEvent::TextChunk(t) => {
                        print!("{t}");
                        io::stdout().flush()?;
                    }
                    StreamEvent::ReasoningChunk(_) => {
                        // skip thinking tokens in this example
                    }
                    StreamEvent::ToolCall(call) => {
                        println!("\n  [calling {}...]", call.name);
                    }
                    StreamEvent::ToolResult(res) => {
                        println!(
                            "  [{} → {}]",
                            res.name,
                            truncate(&res.result.to_string(), 80)
                        );
                    }
                    StreamEvent::Paused(reason) => {
                        paused_reason = Some(reason);
                        break;
                    }
                    StreamEvent::Done(_) => {
                        // end of turn
                    }
                    _ => {}
                }
            }
        }

        match paused_reason {
            Some(reason) => {
                resolve_pending(&reason, messages);
                // Loop around: re-enter chat.stream() with the resolved
                // tools still on the last Content. The pre-step pass
                // runs them and then continues the model's response.
            }
            None => {
                println!();
                return Ok(());
            }
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let end = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
    format!("{}…", &s[..end])
}

// ── 6. Main REPL ──────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let raw_tools: ToolCollection<ApprovalMeta> = ToolCollection::collect_tools()?;
    let scoped = ScopedCollection::new(raw_tools, approval_strategy);

    let client = OpenAIBuilder::new().with_model("gpt-4o-mini").build();

    let mut chat = ChatBuilder::new()
        .with_model(client)
        .with_scoped_tools(scoped)
        .with_max_steps(8)
        .build();

    let mut messages = Messages::default();
    messages.push(content::from_system(parts![
        "You are a terse assistant with destructive tools available. \
         Use them when asked. Explain what you did afterwards.",
    ]));

    println!("── openai HITL chat (ctrl-D or empty line to exit) ──");

    loop {
        let user_input = prompt("\nyou: ")?;
        if user_input.is_empty() {
            break;
        }
        messages.push(content::from_user(parts![&user_input]));

        run_turn(&mut chat, &mut messages).await?;
    }

    println!("\n── session ended ──\n");
    println!("--- final tool audit ---");
    for c in &messages.0 {
        for t in c.parts.tools() {
            let state = match &t.status {
                ToolStatus::Completed { response } => {
                    format!("completed → {}", truncate(&response.result.to_string(), 80))
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
