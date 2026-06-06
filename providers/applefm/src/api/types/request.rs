//! `Messages` + `ChatOptions` → `CompleteRequest` JSON.
//!
//! All capability rejections live here, mistralrs-style: what the
//! on-device model can't do fails fast with a clear error instead of
//! crossing the bridge.

use chat_core::error::{ChatError, ChatFailure};
use chat_core::types::messages::Messages;
use chat_core::types::messages::content::RoleEnum;
use chat_core::types::messages::parts::PartEnum;
use chat_core::types::options::ChatOptions;

use crate::client::{Config, Sampling};

use super::{CompleteRequest, WireMessage, WireOptions};

pub(crate) fn from_core(
    config: &Config,
    messages: &Messages,
    options: Option<&ChatOptions>,
    structured_output: Option<&schemars::Schema>,
    tools_present: bool,
) -> Result<String, ChatFailure> {
    if tools_present {
        return Err(unsupported("tool declarations"));
    }
    if structured_output.is_some() {
        return Err(unsupported("structured outputs"));
    }

    // System-role messages fold into the session instructions; the
    // FoundationModels API has no system role in the conversation itself.
    let mut instructions = String::new();
    let mut wire_messages = Vec::new();

    for content in &messages.0 {
        let text = flatten_text_only(&content.parts.0)?;
        match content.role {
            RoleEnum::System => {
                if !instructions.is_empty() {
                    instructions.push('\n');
                }
                instructions.push_str(&text);
            }
            RoleEnum::User => wire_messages.push(WireMessage { role: "user", text }),
            RoleEnum::Model => wire_messages.push(WireMessage {
                role: "assistant",
                text,
            }),
        }
    }

    if wire_messages.is_empty() {
        return Err(ChatFailure::from_err(ChatError::Provider(
            "chat-applefm needs at least one user message".into(),
        )));
    }

    let request = CompleteRequest {
        instructions: (!instructions.is_empty()).then_some(instructions),
        lora: config
            .lora
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned()),
        messages: wire_messages,
        options: merge_options(config, options),
    };

    serde_json::to_string(&request)
        .map_err(|e| ChatFailure::from_err(ChatError::Other(format!("request serialization: {e}"))))
}

/// Builder defaults overlaid with per-call `ChatOptions`. Temperature and
/// max_tokens merge per field. Sampling merges as a family: if the call
/// carries *any* sampling key (`top_p`, or `greedy`/`top_k`/`seed` in
/// metadata) it replaces the builder's sampling default wholesale, so a
/// builder top-k never mixes with a per-call top-p.
fn merge_options(config: &Config, opts: Option<&ChatOptions>) -> Option<WireOptions> {
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

    fn config() -> Config {
        Config {
            lora: Some("adapters/transcripts.fmadapter".into()),
            ..Default::default()
        }
    }

    #[test]
    fn folds_system_into_instructions_and_carries_lora() {
        let mut messages = Messages::default();
        messages.push(content::from_system(parts!["Talk like a pirate."]));
        messages.push(content::from_user(parts!["hello"]));

        let json = from_core(&config(), &messages, None, None, false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(v["instructions"], "Talk like a pirate.");
        assert_eq!(v["lora"], "adapters/transcripts.fmadapter");
        assert_eq!(v["messages"][0]["role"], "user");
        assert_eq!(v["messages"][0]["text"], "hello");
    }

    #[test]
    fn rejects_tools_and_structured() {
        let mut messages = Messages::default();
        messages.push(content::from_user(parts!["hi"]));

        assert!(from_core(&config(), &messages, None, None, true).is_err());
        let schema = schemars::json_schema!({"type": "object"});
        assert!(from_core(&config(), &messages, None, Some(&schema), false).is_err());
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
        let mut messages = Messages::default();
        messages.push(content::from_user(parts!["hi"]));

        // No call options → builder defaults flow through.
        let json = from_core(&config, &messages, None, None, false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["options"]["top_k"], 40);
        assert_eq!(v["options"]["seed"], 7);

        // A call-level top_p replaces the whole sampling family (no
        // leftover top_k/seed) but per-field merge keeps max_tokens.
        // 0.75 and 0.5 are exact in binary — immune to f32→f64 widening.
        let mut opts = ChatOptions::default();
        opts.top_p = Some(0.75);
        opts.temperature = Some(0.5);
        let json = from_core(&config, &messages, Some(&opts), None, false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["options"]["top_p"], 0.75);
        assert!(v["options"]["top_k"].is_null());
        assert!(v["options"]["seed"].is_null());
        assert_eq!(v["options"]["max_tokens"], 100);
        assert_eq!(v["options"]["temperature"], 0.5);
    }

    #[test]
    fn maps_options() {
        let mut messages = Messages::default();
        messages.push(content::from_user(parts!["hi"]));

        let mut opts = ChatOptions::default();
        opts.temperature = Some(0.2);
        opts.max_tokens = Some(64);
        opts.metadata
            .insert("greedy".into(), serde_json::Value::Bool(true));

        let json = from_core(&config(), &messages, Some(&opts), None, false).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["options"]["max_tokens"], 64);
        assert_eq!(v["options"]["greedy"], true);
    }
}
