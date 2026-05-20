use std::marker::PhantomData;
use std::sync::Arc;

use chat_core::error::{ChatError, ChatFailure};
use chat_core::types::provider_meta::ProviderMeta;
use mistralrs::{GgufModelBuilder, ModelBuilder};

use crate::client::MistralRsClient;

/// Typestate marker — no model id set yet, `build()` is not callable.
pub struct WithoutModel;
/// Typestate marker — model id is set, `build()` is callable.
pub struct WithModel;

/// Device on which to run inference. `Auto` picks the best available
/// backend that was compiled in: Metal on macOS with the `metal` feature,
/// CUDA on Linux with the `cuda` feature, else CPU.
#[derive(Debug, Clone, Copy, Default)]
pub enum DeviceChoice {
    #[default]
    Auto,
    Cpu,
    /// First CUDA device.
    Cuda,
    /// Specific CUDA device by ordinal.
    CudaOrdinal(usize),
    /// Apple Metal (macOS only).
    Metal,
}

/// Builder for [`MistralRsClient`].
///
/// `with_model` sets the Hugging Face repo id (e.g. `Qwen/Qwen2.5-3B-Instruct`).
/// Calling `with_gguf_file` additionally selects a specific GGUF filename
/// inside that repo, which switches the loader to the GGUF path; without
/// it, the auto-detect loader runs against `config.json`.
pub struct MistralRsBuilder<M = WithoutModel> {
    model_id: Option<String>,
    gguf_file: Option<String>,
    tok_model_id: Option<String>,
    device: DeviceChoice,
    description: Option<String>,
    _m: PhantomData<M>,
}

impl Default for MistralRsBuilder<WithoutModel> {
    fn default() -> Self {
        Self::new()
    }
}

impl MistralRsBuilder<WithoutModel> {
    pub fn new() -> Self {
        Self {
            model_id: None,
            gguf_file: None,
            tok_model_id: None,
            device: DeviceChoice::Auto,
            description: None,
            _m: PhantomData,
        }
    }

    /// Set the Hugging Face repo id. Transitions the builder so `.build()`
    /// becomes callable.
    pub fn with_model(self, id: impl Into<String>) -> MistralRsBuilder<WithModel> {
        MistralRsBuilder {
            model_id: Some(id.into()),
            gguf_file: self.gguf_file,
            tok_model_id: self.tok_model_id,
            device: self.device,
            description: self.description,
            _m: PhantomData,
        }
    }
}

impl<M> MistralRsBuilder<M> {
    /// Select a specific `.gguf` file inside the repo and use the GGUF
    /// loader. Required for GGUF-only repos; omit for safetensors repos
    /// where the auto-detect loader handles file discovery.
    pub fn with_gguf_file(mut self, file: impl Into<String>) -> Self {
        self.gguf_file = Some(file.into());
        self
    }

    /// Override the tokenizer source. Useful for GGUF repos that don't
    /// ship `tokenizer.json` — point this at the original safetensors
    /// repo (e.g. `Qwen/Qwen2.5-3B-Instruct`).
    pub fn with_tok_model_id(mut self, id: impl Into<String>) -> Self {
        self.tok_model_id = Some(id.into());
        self
    }

    pub fn with_device(mut self, device: DeviceChoice) -> Self {
        self.device = device;
        self
    }

    pub fn with_description(mut self, d: impl Into<String>) -> Self {
        self.description = Some(d.into());
        self
    }
}

impl MistralRsBuilder<WithModel> {
    /// Load weights and return a ready-to-use client.
    ///
    /// This is async and fallible because it actually downloads (on first
    /// run) and loads the model into memory — multi-second on a warm cache,
    /// multi-minute on a cold one.
    pub async fn build(self) -> Result<MistralRsClient, ChatFailure> {
        let model_id = self.model_id.expect("with_model() sets model_id");
        let force_cpu = matches!(self.device, DeviceChoice::Cpu);

        let model = if let Some(gguf_file) = self.gguf_file.clone() {
            let mut b = GgufModelBuilder::new(model_id.clone(), vec![gguf_file]);
            if let Some(tok) = self.tok_model_id.clone() {
                b = b.with_tok_model_id(tok);
            }
            if force_cpu {
                b = b.with_force_cpu();
            }
            b.build()
                .await
                .map_err(|e| build_failure("GGUF loader", &model_id, e))?
        } else {
            let mut b = ModelBuilder::new(model_id.clone());
            if force_cpu {
                b = b.with_force_cpu();
            }
            b.build()
                .await
                .map_err(|e| build_failure("auto-detect loader", &model_id, e))?
        };

        let meta = Arc::new(ProviderMeta {
            description: self.description,
            ..Default::default()
        });

        Ok(MistralRsClient {
            model: Arc::new(model),
            model_id,
            meta,
        })
    }
}

fn build_failure(loader: &str, model_id: &str, err: anyhow::Error) -> ChatFailure {
    ChatFailure::from_err(ChatError::Provider(format!(
        "mistral.rs {loader} failed to load {model_id}: {err}"
    )))
}
