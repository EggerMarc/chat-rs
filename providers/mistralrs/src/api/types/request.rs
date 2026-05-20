use chat_core::error::{ChatError, ChatFailure};
use chat_core::types::messages::Messages;
use chat_core::types::messages::content::RoleEnum;
use chat_core::types::messages::parts::PartEnum;
use chat_core::types::options::ChatOptions;
use mistralrs::{
    RequestBuilder, SamplingParams, StopTokens, TextMessageRole, TextMessages,
};

/// Build a mistral.rs [`RequestBuilder`] from chat-rs's `Messages` +
/// `ChatOptions`. Phase 1 supports text parts only; other part types
/// produce a clear `ChatFailure` rather than a silent drop.
pub fn from_core(
    messages: &Messages,
    options: Option<&ChatOptions>,
    structured_output: Option<&schemars::Schema>,
    tools_present: bool,
) -> Result<RequestBuilder, ChatFailure> {
    if tools_present {
        return Err(unsupported("tool declarations", "Phase 4"));
    }
    if structured_output.is_some() {
        return Err(unsupported("structured outputs", "Phase 3"));
    }

    let mut txt = TextMessages::new();
    for content in &messages.0 {
        let role = map_role(&content.role);
        let body = flatten_text_parts(&content.parts.0)?;
        txt = txt.add_message(role, body);
    }

    let mut rb: RequestBuilder = txt.into();
    rb = rb.set_sampling(sampling_from_options(options));
    Ok(rb)
}

fn map_role(role: &RoleEnum) -> TextMessageRole {
    match role {
        RoleEnum::User => TextMessageRole::User,
        RoleEnum::System => TextMessageRole::System,
        RoleEnum::Model => TextMessageRole::Assistant,
    }
}

/// Join all `Text` parts in a `Content` with newlines. Any non-text part
/// is rejected — Phase 1 is text-only.
fn flatten_text_parts(parts: &[PartEnum]) -> Result<String, ChatFailure> {
    let mut buf = String::new();
    for part in parts {
        match part {
            PartEnum::Text(t) => {
                if !buf.is_empty() {
                    buf.push('\n');
                }
                buf.push_str(t.as_str());
            }
            PartEnum::File(_) => return Err(unsupported("image / file parts", "Phase 2")),
            PartEnum::Tool(_) => return Err(unsupported("tool parts", "Phase 4")),
            PartEnum::Structured(_) => {
                return Err(unsupported("structured parts in input", "Phase 3"))
            }
            PartEnum::Reasoning(_) => {
                return Err(unsupported("reasoning parts in input", "later phase"))
            }
            PartEnum::Embeddings(_) => {
                return Err(unsupported("embedding parts in input", "later phase"))
            }
        }
    }
    Ok(buf)
}

fn sampling_from_options(options: Option<&ChatOptions>) -> SamplingParams {
    // Start from mistral.rs's deterministic default, then override what the caller set.
    let mut sp = SamplingParams::deterministic();
    let Some(opts) = options else {
        return sp;
    };

    if let Some(t) = opts.temperature {
        sp.temperature = Some(t as f64);
    }
    if let Some(p) = opts.top_p {
        sp.top_p = Some(p as f64);
    }
    if let Some(m) = opts.max_tokens {
        sp.max_len = Some(m as usize);
    }

    // Provider-specific knobs ride through `ChatOptions::metadata`.
    if let Some(k) = opts.metadata.get("top_k").and_then(|v| v.as_u64()) {
        sp.top_k = Some(k as usize);
    }
    if let Some(stops) = opts.metadata.get("stop").and_then(|v| v.as_array()) {
        let seqs: Vec<String> = stops
            .iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect();
        if !seqs.is_empty() {
            sp.stop_toks = Some(StopTokens::Seqs(seqs));
        }
    }
    if let Some(r) = opts.metadata.get("repetition_penalty").and_then(|v| v.as_f64()) {
        sp.repetition_penalty = Some(r as f32);
    }

    sp
}

fn unsupported(what: &str, phase: &str) -> ChatFailure {
    ChatFailure::from_err(ChatError::Provider(format!(
        "chat-mistralrs does not yet support {what} (lands in {phase})"
    )))
}
