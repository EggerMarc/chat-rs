use std::path::PathBuf;
use std::sync::Arc;

use chat_core::types::provider_meta::ProviderMeta;

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

/// Model wiring baked in by the builder and sent to the bridge with
/// every request (sessions are rebuilt per call for now). Conversation
/// concerns — including system prompts — live in `Messages`, not here.
/// Everything below is a default; per-call `ChatOptions` override it.
#[derive(Debug, Default)]
pub(crate) struct Config {
    /// Path to a `.fmadapter` package — a LoRA trained with Apple's
    /// adapter training toolkit, applied over the on-device base model.
    pub(crate) lora: Option<PathBuf>,
    pub(crate) temperature: Option<f64>,
    pub(crate) max_tokens: Option<u32>,
    pub(crate) sampling: Option<Sampling>,
}

/// Client for the Apple Intelligence on-device foundation model.
///
/// There is no model slug and no API key: the OS owns the (one) model.
/// What varies per client is the configuration — instructions and an
/// optional LoRA adapter. Clone is cheap (`Arc` bumps).
#[derive(Clone, Debug)]
pub struct AppleFMClient {
    pub(crate) config: Arc<Config>,
    /// `Arc` because `ProviderMeta` is not `Clone`.
    pub(crate) meta: Arc<ProviderMeta>,
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
}
