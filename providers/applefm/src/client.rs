use std::path::PathBuf;
use std::sync::Arc;

use chat_core::types::provider_meta::ProviderMeta;

use crate::api::types::request::{ConvoEntry, TurnPlan, hash_convo};
use crate::ffi;

/// Decoding strategy for the on-device model — the full set
/// FoundationModels exposes via `GenerationOptions.SamplingMode`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sampling {
    /// Deterministic decoding.
    Greedy,
    /// Sample among the `k` most probable tokens.
    TopK { k: u32, seed: Option<u64> },
    /// Nucleus sampling: sample within the smallest set of tokens whose
    /// cumulative probability reaches `p`.
    TopP { p: f64, seed: Option<u64> },
}

/// Model wiring baked in by the builder. Conversation concerns —
/// including system prompts — live in `Messages`, not here. Everything
/// below `lora` is a default; per-call `ChatOptions` override it.
#[derive(Debug, Default)]
pub(crate) struct Config {
    /// Path to a `.fmadapter` package — a LoRA trained with Apple's
    /// adapter training toolkit, applied over the on-device base model.
    pub(crate) lora: Option<PathBuf>,
    pub(crate) temperature: Option<f64>,
    pub(crate) max_tokens: Option<u32>,
    pub(crate) sampling: Option<Sampling>,
}

/// Owned handle to a live bridge session; releases it on drop.
#[derive(Debug)]
struct SessionHandle(u64);

impl Drop for SessionHandle {
    fn drop(&mut self) {
        ffi::session_free(self.0);
    }
}

#[derive(Debug)]
struct SessionState {
    handle: SessionHandle,
    instructions_hash: u64,
    /// Hash of every conversation entry the session has seen, including
    /// model replies.
    prefix_hash: u64,
    prefix_len: usize,
}

/// The client's slot for a live bridge session, with the fingerprint
/// that recognizes append-only continuations. All lifecycle transitions
/// go through these methods; nothing else touches the state.
#[derive(Debug, Default)]
pub(crate) struct Session(Option<SessionState>);

impl Session {
    /// Decide how to run this turn: incremental prefill against the held
    /// session, or tear down and rebuild. Reuse requires the same
    /// instructions and a conversation that extends what the session has
    /// seen by exactly one message.
    pub(crate) fn plan(&self, instructions_hash: u64, convo: &[ConvoEntry]) -> TurnPlan {
        match &self.0 {
            Some(s)
                if s.instructions_hash == instructions_hash
                    && convo.len() == s.prefix_len + 1
                    && hash_convo(&convo[..s.prefix_len]) == s.prefix_hash =>
            {
                TurnPlan::Reuse
            }
            _ => TurnPlan::Rebuild,
        }
    }

    /// The live bridge session id, if any.
    pub(crate) fn id(&self) -> Option<u64> {
        self.0.as_ref().map(|s| s.handle.0)
    }

    /// Drop the held session (frees the bridge side). Called on rebuild
    /// and whenever a turn errors — the bridge session may then hold a
    /// half-applied turn, so the next call starts fresh.
    pub(crate) fn invalidate(&mut self) {
        self.0 = None;
    }

    /// Replace with a freshly created bridge session that has seen
    /// nothing yet.
    pub(crate) fn install(&mut self, id: u64, instructions_hash: u64) {
        self.0 = Some(SessionState {
            handle: SessionHandle(id),
            instructions_hash,
            prefix_hash: hash_convo(&[]),
            prefix_len: 0,
        });
    }

    /// Record that the session has now seen `convo` plus the model's
    /// reply, so the next append-only turn plans as `Reuse`.
    pub(crate) fn advance(&mut self, mut convo: Vec<ConvoEntry>, reply_text: String) {
        if let Some(s) = &mut self.0 {
            convo.push(ConvoEntry {
                role: "assistant",
                text: reply_text,
            });
            s.prefix_len = convo.len();
            s.prefix_hash = hash_convo(&convo);
        }
    }
}

/// Client for the Apple Intelligence on-device foundation model.
///
/// There is no model slug and no API key: the OS owns the (one) model.
/// What varies per client is the configuration — a LoRA adapter and
/// decoding defaults.
///
/// The client holds a live session across turns: append-only
/// conversations prefill only the newest message, which is what makes
/// multi-turn chat fast. Clones share that session, so use one client
/// per concurrent conversation.
#[derive(Clone, Debug)]
pub struct AppleFMClient {
    pub(crate) config: Arc<Config>,
    /// `Arc` because `ProviderMeta` is not `Clone`.
    pub(crate) meta: Arc<ProviderMeta>,
    /// Locked for the whole of each turn — serializes calls so the
    /// bridge session is never used concurrently.
    pub(crate) session: Arc<tokio::sync::Mutex<Session>>,
}

impl AppleFMClient {
    /// Identifier used as the response `model_slug`: the base model name,
    /// plus the adapter file stem when a LoRA is loaded.
    pub fn model_slug(&self) -> String {
        match self.config.lora.as_deref().and_then(|p| p.file_stem()) {
            Some(stem) => format!("apple-on-device+{}", stem.to_string_lossy()),
            None => "apple-on-device".to_owned(),
        }
    }

    pub fn provider_meta(&self) -> &ProviderMeta {
        &self.meta
    }

    /// Hint the OS to stage model resources for an upcoming turn.
    ///
    /// The runtime stages the model down between requests, so a turn that
    /// follows an idle pause (a user typing, say) pays seconds of warm-up.
    /// Call this when you can predict a turn is coming — on input focus,
    /// when the user starts typing — and the warm-up overlaps the wait.
    ///
    /// Fire-and-forget and cheap: returns immediately, never fails, and
    /// is skipped entirely if a turn is already in flight (the model is
    /// active then anyway). Works before the first turn too.
    pub fn prewarm(&self) {
        if let Ok(session) = self.session.try_lock() {
            let id = session.id().unwrap_or(0);
            drop(session);
            ffi::prewarm(id);
        }
    }

    /// Fill the metadata the bridge can't: wall-clock duration and
    /// creation time measured around the call, the active LoRA adapter,
    /// and whether the turn reused the session (`prefill:
    /// "incremental"`) or rebuilt it (`prefill: "full"`). Token usage
    /// stays zero — Apple's FoundationModels API does not expose token
    /// counts.
    pub(crate) fn enrich_metadata(
        &self,
        metadata: &mut chat_core::types::metadata::Metadata,
        elapsed: std::time::Duration,
        reused_session: bool,
    ) {
        metadata.duration_ms = Some(elapsed.as_millis() as u64);
        metadata.created_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs());
        metadata.provider_specific.insert(
            "prefill".to_owned(),
            serde_json::Value::String(
                if reused_session {
                    "incremental"
                } else {
                    "full"
                }
                .to_owned(),
            ),
        );
        if let Some(lora) = &self.config.lora {
            metadata.provider_specific.insert(
                "lora".to_owned(),
                serde_json::Value::String(lora.to_string_lossy().into_owned()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::request::hash_instructions;

    fn entry(role: &'static str, text: &str) -> ConvoEntry {
        ConvoEntry {
            role,
            text: text.to_owned(),
        }
    }

    #[test]
    fn session_lifecycle_plans_correctly() {
        let instructions_hash = hash_instructions(Some("sys"));
        let mut session = Session::default();

        // No session yet → rebuild.
        let convo1 = vec![entry("user", "hi")];
        assert_eq!(session.plan(instructions_hash, &convo1), TurnPlan::Rebuild);

        // Simulate the first turn: install, then advance with the reply.
        session.install(1, instructions_hash);
        session.advance(convo1, "yo".to_owned());

        // Append-only next turn → reuse.
        let convo2 = vec![
            entry("user", "hi"),
            entry("assistant", "yo"),
            entry("user", "how are you?"),
        ];
        assert_eq!(session.plan(instructions_hash, &convo2), TurnPlan::Reuse);

        // Edited history → rebuild.
        let edited = vec![
            entry("user", "hi EDITED"),
            entry("assistant", "yo"),
            entry("user", "how are you?"),
        ];
        assert_eq!(session.plan(instructions_hash, &edited), TurnPlan::Rebuild);

        // Changed instructions → rebuild.
        assert_eq!(
            session.plan(hash_instructions(Some("other")), &convo2),
            TurnPlan::Rebuild
        );

        // Invalidated → rebuild.
        session.invalidate();
        assert_eq!(session.id(), None);
        assert_eq!(session.plan(instructions_hash, &convo2), TurnPlan::Rebuild);
    }
}
