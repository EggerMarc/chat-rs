use chat_core::error::{ChatError, ChatFailure};
use chat_core::types::messages::Messages;
use chat_core::types::messages::content::RoleEnum;
use chat_core::types::messages::file::{File, FileSource};
use chat_core::types::messages::parts::PartEnum;
use chat_core::types::options::ChatOptions;
use image::DynamicImage;
use mistralrs::{
    AudioInput, MultimodalMessages, RequestBuilder, SamplingParams, StopTokens, TextMessageRole,
    TextMessages,
};

/// Build a mistral.rs [`RequestBuilder`] from chat-rs's `Messages` +
/// `ChatOptions`.
///
/// Picks between the text and multimodal message paths automatically: any
/// image or audio `File` part anywhere in `messages` flips on the
/// multimodal path. Other part types (tool, structured, reasoning,
/// embeddings) are still rejected with `ChatFailure` — they'll land in
/// their own phases.
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

    let has_media = messages
        .0
        .iter()
        .any(|c| c.parts.0.iter().any(is_media_part));
    let rb: RequestBuilder = if has_media {
        build_multimodal(messages)?.into()
    } else {
        build_text(messages)?.into()
    };

    Ok(rb.set_sampling(sampling_from_options(options)))
}

fn build_text(messages: &Messages) -> Result<TextMessages, ChatFailure> {
    let mut txt = TextMessages::new();
    for content in &messages.0 {
        let role = map_role(&content.role);
        let body = flatten_text_only(&content.parts.0)?;
        txt = txt.add_message(role, body);
    }
    Ok(txt)
}

fn build_multimodal(messages: &Messages) -> Result<MultimodalMessages, ChatFailure> {
    let mut mm = MultimodalMessages::new();
    for content in &messages.0 {
        let role = map_role(&content.role);
        let (text, images, audio) = split_text_and_media(&content.parts.0)?;
        if images.is_empty() && audio.is_empty() {
            mm = mm.add_message(role, text);
        } else {
            mm = mm.add_multimodal_message(role, text, images, audio, vec![]);
        }
    }
    Ok(mm)
}

fn map_role(role: &RoleEnum) -> TextMessageRole {
    match role {
        RoleEnum::User => TextMessageRole::User,
        RoleEnum::System => TextMessageRole::System,
        RoleEnum::Model => TextMessageRole::Assistant,
    }
}

/// Collect all `Text` parts into one string (newline-joined). Reject any
/// non-text file or other unsupported part with a clear phase-specific
/// error. Only called on the text-only path, where media parts are absent
/// by construction.
fn flatten_text_only(parts: &[PartEnum]) -> Result<String, ChatFailure> {
    let mut buf = String::new();
    for part in parts {
        match part {
            PartEnum::Text(t) => append_line(&mut buf, t.as_str()),
            PartEnum::File(f) => {
                return Err(unsupported(
                    &format!("file parts with mimetype {}", f.mime),
                    "later phase (video, documents)",
                ));
            }
            PartEnum::Tool(_) => return Err(unsupported("tool parts", "Phase 4")),
            PartEnum::Structured(_) => {
                return Err(unsupported("structured parts in input", "Phase 3"));
            }
            PartEnum::Reasoning(_) => {
                return Err(unsupported("reasoning parts in input", "later phase"));
            }
            PartEnum::Embeddings(_) => {
                return Err(unsupported("embedding parts in input", "later phase"));
            }
        }
    }
    Ok(buf)
}

fn split_text_and_media(
    parts: &[PartEnum],
) -> Result<(String, Vec<DynamicImage>, Vec<AudioInput>), ChatFailure> {
    let mut text = String::new();
    let mut images = Vec::new();
    let mut audio = Vec::new();
    for part in parts {
        match part {
            PartEnum::Text(t) => append_line(&mut text, t.as_str()),
            PartEnum::File(f) if f.is_image() => {
                images.push(decode_image(f)?);
            }
            PartEnum::File(f) if f.is_audio() => {
                audio.push(decode_audio(f)?);
            }
            PartEnum::File(f) => {
                return Err(unsupported(
                    &format!("file parts with mimetype {}", f.mime),
                    "later phase (video, documents)",
                ));
            }
            PartEnum::Tool(_) => return Err(unsupported("tool parts", "Phase 4")),
            PartEnum::Structured(_) => {
                return Err(unsupported("structured parts in input", "Phase 3"));
            }
            PartEnum::Reasoning(_) => {
                return Err(unsupported("reasoning parts in input", "later phase"));
            }
            PartEnum::Embeddings(_) => {
                return Err(unsupported("embedding parts in input", "later phase"));
            }
        }
    }
    Ok((text, images, audio))
}

fn append_line(buf: &mut String, s: &str) {
    if !buf.is_empty() {
        buf.push('\n');
    }
    buf.push_str(s);
}

fn is_media_part(part: &PartEnum) -> bool {
    matches!(part, PartEnum::File(f) if f.is_image() || f.is_audio())
}

fn decode_image(file: &File) -> Result<DynamicImage, ChatFailure> {
    match &file.source {
        FileSource::Bytes(bytes) => image::load_from_memory(bytes).map_err(|e| {
            ChatFailure::from_err(ChatError::InvalidResponse(format!(
                "could not decode image bytes ({}): {e}",
                file.mime
            )))
        }),
        FileSource::Url(_) => Err(ChatFailure::from_err(ChatError::Provider(
            "remote-URL images are not supported yet — fetch bytes and pass them via \
             File::from_bytes"
                .into(),
        ))),
    }
}

fn decode_audio(file: &File) -> Result<AudioInput, ChatFailure> {
    match &file.source {
        FileSource::Bytes(bytes) => AudioInput::from_bytes(bytes).map_err(|e| {
            ChatFailure::from_err(ChatError::InvalidResponse(format!(
                "could not decode audio bytes: {e}"
            )))
        }),
        FileSource::Url(_) => Err(ChatFailure::from_err(ChatError::Provider(
            "remote-URL audio is not supported yet — pass bytes via File::from_bytes".into(),
        ))),
    }
}

fn sampling_from_options(options: Option<&ChatOptions>) -> SamplingParams {
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
    if let Some(r) = opts
        .metadata
        .get("repetition_penalty")
        .and_then(|v| v.as_f64())
    {
        sp.repetition_penalty = Some(r as f32);
    }

    sp
}

fn unsupported(what: &str, phase: &str) -> ChatFailure {
    ChatFailure::from_err(ChatError::Provider(format!(
        "chat-mistralrs does not yet support {what} (lands in {phase})"
    )))
}
