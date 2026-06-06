use std::path::PathBuf;
use std::sync::Arc;

use chat_core::types::provider_meta::ProviderMeta;

use crate::client::{AppleFMClient, Config, Sampling};

/// Builder for [`AppleFMClient`].
///
/// A pure connection stub: it wires up *which model variant* to talk to
/// (base, or base + LoRA) and nothing else. Unlike other providers there
/// is no `with_model` typestate — the model is the one the OS ships.
/// Conversation concerns (system prompts, options) belong to the chat:
/// push a `System`-role message into `Messages` and the provider maps it
/// onto the session's instructions.
///
/// ```no_run
/// use chat_applefm::AppleFMBuilder;
///
/// let client = AppleFMBuilder::new()
///     .with_lora("adapters/transcripts.fmadapter")
///     .build();
/// ```
#[derive(Debug, Default)]
pub struct AppleFMBuilder {
    lora: Option<PathBuf>,
    temperature: Option<f64>,
    max_tokens: Option<u32>,
    sampling: Option<Sampling>,
    description: Option<String>,
}

impl AppleFMBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply a LoRA fine-tune over the on-device base model: the path to a
    /// `.fmadapter` package produced by Apple's adapter training toolkit.
    ///
    /// Adapters are tied to a specific base-model version — when a macOS
    /// update rolls the base model, the adapter must be retrained. Loading
    /// an incompatible adapter fails at request time with a provider error.
    pub fn with_lora(mut self, path: impl Into<PathBuf>) -> Self {
        self.lora = Some(path.into());
        self
    }

    /// Default decoding temperature. Overridden per call by
    /// `ChatOptions::temperature`.
    pub fn with_temperature(mut self, temperature: f64) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Default response-length cap. Overridden per call by
    /// `ChatOptions::max_tokens`.
    pub fn with_max_tokens(mut self, max_tokens: u32) -> Self {
        self.max_tokens = Some(max_tokens);
        self
    }

    /// Default sampling mode — greedy, top-k, or top-p (the complete set
    /// FoundationModels exposes). Overridden per call when `ChatOptions`
    /// carries any sampling key (`top_p`, or `greedy` / `top_k` / `seed`
    /// in its metadata).
    pub fn with_sampling(mut self, sampling: Sampling) -> Self {
        self.sampling = Some(sampling);
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Build the client. Infallible and cheap — the model is probed and
    /// the session created at request time. Call
    /// [`crate::availability`] first to know whether requests can succeed
    /// on this machine at all.
    pub fn build(self) -> AppleFMClient {
        AppleFMClient {
            config: Arc::new(Config {
                lora: self.lora,
                temperature: self.temperature,
                max_tokens: self.max_tokens,
                sampling: self.sampling,
            }),
            meta: Arc::new(ProviderMeta {
                description: self.description,
                ..Default::default()
            }),
        }
    }
}
