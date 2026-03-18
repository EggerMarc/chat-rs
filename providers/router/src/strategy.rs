use async_trait::async_trait;
use chat_core::types::messages::Messages;
use chat_core::types::provider_meta::ProviderMeta;

pub type StrategyError = Box<dyn std::error::Error + Send + Sync>;

#[async_trait]
pub trait RoutingStrategy: Send + Sync {
    async fn rank(
        &self,
        messages: &Messages,
        providers: &[Option<&ProviderMeta>],
    ) -> Result<Vec<usize>, StrategyError>;
}
