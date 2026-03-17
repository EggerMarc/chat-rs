use async_trait::async_trait;
use chat_core::traits::CompletionProvider;
use chat_core::types::messages::Messages;

pub type StrategyError = Box<dyn std::error::Error + Send + Sync>;

#[async_trait]
pub trait RoutingStrategy: Send + Sync {
    async fn rank(
        &self,
        messages: &Messages,
        providers: &[Box<dyn CompletionProvider>],
    ) -> Result<Vec<usize>, StrategyError>;
}
