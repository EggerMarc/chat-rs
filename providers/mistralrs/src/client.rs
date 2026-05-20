use std::sync::Arc;

use chat_core::types::provider_meta::ProviderMeta;
use mistralrs::Model;

/// Local-inference client backed by a loaded mistral.rs `Model`.
///
/// Clone is cheap (`Arc` bumps). Multiple clones share the same loaded
/// weights — call `complete()` on each clone to run multiple requests
/// concurrently against the same engine.
#[derive(Clone)]
pub struct MistralRsClient {
    pub(crate) model: Arc<Model>,
    pub(crate) model_id: String,
    /// Wrapped in `Arc` because `ProviderMeta` is not `Clone` (it holds a
    /// `HashMap<String, Box<dyn Any + Send + Sync>>`).
    pub(crate) meta: Arc<ProviderMeta>,
}

impl MistralRsClient {
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    pub fn provider_meta(&self) -> &ProviderMeta {
        &self.meta
    }
}
