//! `Messages` + `ChatOptions` → the session/turn wire JSON, plus the
//! turn-planning logic that decides between incremental and full
//! prefill.
//!
//! All capability rejections live here, mistralrs-style: what the
//! on-device model can't do fails fast with a clear error instead of
//! crossing the bridge.

use std::hash::{DefaultHasher, Hash, Hasher};

use chat_core::error::{ChatError, ChatFailure};
use chat_core::types::messages::Messages;
use chat_core::types::messages::content::RoleEnum;
use chat_core::types::messages::parts::PartEnum;
use chat_core::types::options::ChatOptions;

use crate::client::{Config, Sampling};

use super::{SessionConfig, TurnRequest, WireOptions};

/// A flattened conversation entry (system messages excluded — they fold
/// into the session instructions).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ConvoEntry {
    pub role: &'static str,
    pub text: String,
}

/// Validate the request and flatten `Messages` into instructions (from
/// `System` roles — FoundationModels has no system role in the
/// conversation itself) plus user/assistant entries.
pub(crate) fn prepare(
    messages: &Messages,
    structured_output: Option<&schemars::Schema>,
    tools_present: bool,
) -> Result<(Option<String>, Vec<ConvoEntry>), ChatFailure> {
    if tools_present {
        return Err(unsupported("tool declarations"));
    }
    if structured_output.is_some() {
        return Err(unsupported("structured outputs"));
    }

    let mut instructions = String::new();
    let mut convo = Vec::new();

    for content in &messages.0 {
        let text = flatten_text_only(&content.parts.0)?;
        match content.role {
            RoleEnum::System => {
                if !instructions.is_empty() {
                    instructions.push('\n');
                }
                instructions.push_str(&text);
            }
            RoleEnum::User => convo.push(ConvoEntry { role: "user", text }),
            RoleEnum::Model => convo.push(ConvoEntry {
                role: "assistant",
                text,
            }),
        }
    }

    if convo.is_empty() {
        return Err(ChatFailure::from_err(ChatError::Provider(
            "chat-applefm needs at least one user message".into(),
        )));
    }

    Ok(((!instructions.is_empty()).then_some(instructions), convo))
}

/// How to run this turn against the held session, if any. Decided by
/// [`crate::client::Session::plan`].
#[derive(Debug, PartialEq)]
pub(crate) enum TurnPlan {
    /// The conversation extends what the session has seen by exactly one
    /// message: send only that message (incremental prefill).
    Reuse,
    /// First turn, edited history, or changed instructions: tear down
    /// and create a fresh session, sending the full conversation.
    Rebuild,
}

pub(crate) fn hash_instructions(instructions: Option<&str>) -> u64 {
    let mut hasher = DefaultHasher::new();
    instructions.hash(&mut hasher);
    hasher.finish()
}

pub(crate) fn hash_convo(entries: &[ConvoEntry]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for entry in entries {
        entry.role.hash(&mut hasher);
        entry.text.hash(&mut hasher);
    }
    hasher.finish()
}

/// Render a full conversation into one prompt (rebuild path). Single
/// user turn passes through untagged.
pub(crate) fn render_full(convo: &[ConvoEntry]) -> String {
    if let [only] = convo {
        return only.text.clone();
    }
    convo
        .iter()
        .map(|entry| {
            let tag = if entry.role == "assistant" {
                "Assistant"
            } else {
                "User"
            };
            format!("{tag}: {}", entry.text)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn session_config_json(
    instructions: Option<&str>,
    config: &Config,
) -> Result<String, ChatFailure> {
    let session_config = SessionConfig {
        instructions: instructions.map(str::to_owned),
        lora: config
            .lora
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
    };
    to_json(&session_config)
}

pub(crate) fn turn_request_json(
    message: String,
    options: Option<WireOptions>,
) -> Result<String, ChatFailure> {
    to_json(&TurnRequest { message, options })
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<String, ChatFailure> {
    serde_json::to_string(value)
        .map_err(|e| ChatFailure::from_err(ChatError::Other(format!("request serialization: {e}"))))
}

/// Builder defaults overlaid with per-call `ChatOptions`. Temperature and
/// max_tokens merge per field. Sampling merges as a family: if the call
/// carries *any* sampling key (`top_p`, or `greedy`/`top_k`/`seed` in
/// metadata) it replaces the builder's sampling default wholesale, so a
/// builder top-k never mixes with a per-call top-p.
pub(crate) fn merge_options(config: &Config, opts: Option<&ChatOptions>) -> Option<WireOptions> {
    let mut wire = WireOptions {
        temperature: config.temperature,
        max_tokens: config.max_tokens,
        ..Default::default()
    };
    match config.sampling {
        Some(Sampling::Greedy) => wire.greedy = Some(true),
        Some(Sampling::TopK { k, seed }) => (wire.top_k, wire.seed) = (Some(k), seed),
        Some(Sampling::TopP { p, seed }) => (wire.top_p, wire.seed) = (Some(p), seed),
        None => {}
    }

    if let Some(opts) = opts {
        if let Some(t) = opts.temperature {
            wire.temperature = Some(f64::from(t));
        }
        if let Some(m) = opts.max_tokens {
            wire.max_tokens = Some(m);
        }

        let greedy = opts.metadata.get("greedy").and_then(|v| v.as_bool());
        let top_k = opts.metadata.get("top_k").and_then(|v| v.as_u64());
        let seed = opts.metadata.get("seed").and_then(|v| v.as_u64());
        if greedy.is_some() || top_k.is_some() || opts.top_p.is_some() || seed.is_some() {
            (wire.greedy, wire.top_k, wire.top_p, wire.seed) = (
                greedy,
                top_k.map(|k| k as u32),
                opts.top_p.map(f64::from),
                seed,
            );
        }
    }

    let is_empty = matches!(
        wire,
        WireOptions {
            temperature: None,
            max_tokens: None,
            greedy: None,
            top_k: None,
            top_p: None,
            seed: None,
        }
    );
    (!is_empty).then_some(wire)
}

/// The on-device model is text-only via this API; everything else is
/// rejected with a clear pointer to what's unsupported.
fn flatten_text_only(parts: &[PartEnum]) -> Result<String, ChatFailure> {
    let mut buf = String::new();
    for part in parts {
        match part {
            PartEnum::Text(t) => {
                if !buf.is_empty() {
                    buf.push('\n');
                }
                buf.push_str(t.as_str());
            }
            PartEnum::File(f) => {
                return Err(unsupported(&format!("file parts (mimetype {})", f.mime)));
            }
            PartEnum::Tool(_) => return Err(unsupported("tool parts")),
            PartEnum::Structured(_) => return Err(unsupported("structured parts in input")),
            PartEnum::Reasoning(_) => return Err(unsupported("reasoning parts in input")),
            PartEnum::Embeddings(_) => return Err(unsupported("embedding parts in input")),
        }
    }
    Ok(buf)
}

fn unsupported(what: &str) -> ChatFailure {
    ChatFailure::from_err(ChatError::Provider(format!(
        "chat-applefm does not yet support {what}"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chat_core::parts;
    use chat_core::types::messages::content;

    fn entry(role: &'static str, text: &str) -> ConvoEntry {
        ConvoEntry {
            role,
            text: text.to_owned(),
        }
    }

    #[test]
    fn folds_system_into_instructions() {
        let mut messages = Messages::default();
        messages.push(content::from_system(parts!["Talk like a pirate."]));
        messages.push(content::from_user(parts!["hello"]));

        let (instructions, convo) = prepare(&messages, None, false).unwrap();
        assert_eq!(instructions.as_deref(), Some("Talk like a pirate."));
        assert_eq!(convo, vec![entry("user", "hello")]);
    }

    #[test]
    fn rejects_tools_and_structured() {
        let mut messages = Messages::default();
        messages.push(content::from_user(parts!["hi"]));

        assert!(prepare(&messages, None, true).is_err());
        let schema = schemars::json_schema!({"type": "object"});
        assert!(prepare(&messages, Some(&schema), false).is_err());
    }

    #[test]
    fn renders_single_and_multi_turn() {
        assert_eq!(render_full(&[entry("user", "hi")]), "hi");
        assert_eq!(
            render_full(&[entry("user", "hi"), entry("assistant", "yo")]),
            "User: hi\n\nAssistant: yo"
        );
    }

    #[test]
    fn builder_defaults_yield_to_call_options_as_a_family() {
        let config = Config {
            lora: None,
            temperature: Some(0.7),
            max_tokens: Some(100),
            sampling: Some(Sampling::TopK {
                k: 40,
                seed: Some(7),
            }),
        };

        // No call options → builder defaults flow through.
        let wire = merge_options(&config, None).unwrap();
        assert_eq!(wire.top_k, Some(40));
        assert_eq!(wire.seed, Some(7));

        // A call-level top_p replaces the whole sampling family (no
        // leftover top_k/seed) but per-field merge keeps max_tokens.
        // 0.75 and 0.5 are exact in binary — immune to f32→f64 widening.
        let mut opts = ChatOptions::default();
        opts.top_p = Some(0.75);
        opts.temperature = Some(0.5);
        let wire = merge_options(&config, Some(&opts)).unwrap();
        assert_eq!(wire.top_p, Some(0.75));
        assert_eq!(wire.top_k, None);
        assert_eq!(wire.seed, None);
        assert_eq!(wire.max_tokens, Some(100));
        assert_eq!(wire.temperature, Some(0.5));
    }

    #[test]
    fn session_config_carries_lora() {
        let config = Config {
            lora: Some("adapters/transcripts.fmadapter".into()),
            ..Default::default()
        };
        let json = session_config_json(Some("sys"), &config).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["instructions"], "sys");
        assert_eq!(v["lora"], "adapters/transcripts.fmadapter");
    }
}
