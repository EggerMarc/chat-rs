//! Integration tests for the `Tool` part refactor.
//!
//! Covers:
//! - Lifecycle transitions
//! - Wire projection via `to_tuple` for every resolved state
//! - `PartEnum::Tool` serde round-trip
//! - `Parts` iteration helpers

use chat_core::types::messages::{
    parts::{PartEnum, Parts},
    tool::{Tool, ToolStatus},
};
use serde_json::json;
use tools_rs::{CallId, FunctionCall, FunctionResponse};

fn fc(name: &str) -> FunctionCall {
    FunctionCall {
        id: Some(CallId::new()),
        name: name.to_string(),
        arguments: json!({"arg": "val"}),
    }
}

#[test]
fn completed_tool_projects_to_real_response() {
    let mut t = Tool::new(fc("ls"));
    t.mark_running();
    t.complete(FunctionResponse {
        id: Some(t.id.clone()),
        name: "ls".to_string(),
        result: json!({"files": ["a", "b"]}),
    });

    let (call, resp) = t.to_tuple();
    assert_eq!(call.name, "ls");
    let resp = resp.expect("completed must produce a response");
    assert_eq!(resp.result["files"], json!(["a", "b"]));
}

#[test]
fn rejected_tool_projects_to_synthetic_error() {
    let mut t = Tool::new(fc("rm"));
    t.reject(Some("forbidden".to_string()));
    let (_, resp) = t.to_tuple();
    let resp = resp.expect("rejected must produce a synth response");
    assert!(resp.result["error"].as_str().unwrap().contains("forbidden"));
}

#[test]
fn failed_tool_projects_to_synthetic_error() {
    let mut t = Tool::new(fc("net"));
    t.fail("connection reset");
    let (_, resp) = t.to_tuple();
    assert_eq!(
        resp.unwrap().result["error"].as_str().unwrap(),
        "connection reset"
    );
}

#[test]
fn pending_approved_running_yield_no_response() {
    let t = Tool::new(fc("x"));
    assert!(t.to_tuple().1.is_none());

    let mut t2 = Tool::new(fc("x"));
    t2.approve(None);
    assert!(t2.to_tuple().1.is_none());

    let mut t3 = Tool::new(fc("x"));
    t3.mark_running();
    assert!(t3.to_tuple().1.is_none());
}

#[test]
fn try_to_tuple_is_strict() {
    let t = Tool::new(fc("x"));
    assert!(t.try_to_tuple().is_err());

    let mut t2 = Tool::new(fc("x"));
    t2.complete(FunctionResponse {
        id: Some(t2.id.clone()),
        name: "x".to_string(),
        result: json!("done"),
    });
    assert!(t2.try_to_tuple().is_ok());
}

#[test]
fn edited_call_wins_in_projection() {
    let mut t = Tool::new(fc("write"));
    let edited = FunctionCall {
        id: t.call.id.clone(),
        name: "write".to_string(),
        arguments: json!({"path": "/safe/path"}),
    };
    t.approve(Some(edited));
    let (call, _) = t.to_tuple();
    assert_eq!(call.arguments, json!({"path": "/safe/path"}));
}

#[test]
fn part_enum_tool_serde_round_trip() {
    let mut t = Tool::new(fc("ls"));
    t.complete(FunctionResponse {
        id: Some(t.id.clone()),
        name: "ls".to_string(),
        result: json!({"ok": true}),
    });

    let part = PartEnum::Tool(t);
    let encoded = serde_json::to_string(&part).unwrap();
    let decoded: PartEnum = serde_json::from_str(&encoded).unwrap();
    assert_eq!(part, decoded);
}

#[test]
fn part_enum_tool_rejected_serde_round_trip() {
    let mut t = Tool::new(fc("rm"));
    t.reject(Some("no way".to_string()));
    let part = PartEnum::Tool(t);
    let encoded = serde_json::to_string(&part).unwrap();
    let decoded: PartEnum = serde_json::from_str(&encoded).unwrap();
    assert_eq!(part, decoded);
}

#[test]
fn parts_iterator_helpers_filter_by_state() {
    let mut parts = Parts::default();
    parts.push(PartEnum::from_text("hi"));

    let pending = Tool::new(fc("a"));
    parts.push(PartEnum::Tool(pending));

    let mut resolved = Tool::new(fc("b"));
    resolved.complete(FunctionResponse {
        id: Some(resolved.id.clone()),
        name: "b".to_string(),
        result: json!("x"),
    });
    parts.push(PartEnum::Tool(resolved));

    assert_eq!(parts.tools().count(), 2);
    assert_eq!(parts.pending_tools().count(), 1);
    assert_eq!(parts.resolved_tools().count(), 1);
}

#[test]
fn tool_status_state_name_matches_variant() {
    assert_eq!(ToolStatus::Pending.state_name(), "Pending");
    assert_eq!(ToolStatus::Running.state_name(), "Running");
    assert_eq!(
        ToolStatus::Completed {
            response: FunctionResponse {
                id: None,
                name: "".into(),
                result: json!(null),
            },
        }
        .state_name(),
        "Completed"
    );
    assert_eq!(
        ToolStatus::Rejected { reason: None }.state_name(),
        "Rejected"
    );
    assert_eq!(
        ToolStatus::Failed {
            error: "".into()
        }
        .state_name(),
        "Failed"
    );
}
