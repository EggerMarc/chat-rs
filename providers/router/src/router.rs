use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::CompletionProvider;
use chat_core::types::messages::Messages;
use chat_core::types::options::ChatOptions;
use chat_core::types::response::ChatResponse;
use tools_rs::ToolCollection;

use crate::strategy::RoutingStrategy;

pub struct Router {
    pub(crate) providers: Vec<Box<dyn CompletionProvider>>,
    pub(crate) strategy: Option<Box<dyn RoutingStrategy>>,
}

#[async_trait::async_trait]
impl CompletionProvider for Router {
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tools: Option<&ToolCollection>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        let count = self.providers.len();
        if count == 0 {
            return Err(ChatFailure::from_err(ChatError::Other(
                "Router has no providers".to_string(),
            )));
        }

        let order: Vec<usize> = match &self.strategy {
            Some(strategy) => strategy.rank(messages, count),
            None => (0..count).collect(),
        };

        let mut last_failure: Option<ChatFailure> = None;

        for idx in order {
            let provider = match self.providers.get_mut(idx) {
                Some(p) => p,
                None => continue,
            };

            match provider
                .complete(messages, tools, options, structured_output)
                .await
            {
                Ok(response) => return Ok(response),
                Err(failure) => {
                    if !failure.err.is_retryable() {
                        return Err(failure);
                    }
                    last_failure = Some(failure);
                }
            }
        }

        Err(last_failure.unwrap_or_else(|| {
            ChatFailure::from_err(ChatError::Other(
                "All providers exhausted".to_string(),
            ))
        }))
    }
}
