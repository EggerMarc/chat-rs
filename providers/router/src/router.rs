use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::CompletionProvider;
use chat_core::types::messages::Messages;
use chat_core::types::options::ChatOptions;
use chat_core::types::provider_meta::ProviderMeta;
use chat_core::types::response::ChatResponse;
use chat_core::types::tools::ToolDeclarations;

use crate::circuit_breaker::CircuitBreaker;
use crate::strategy::RoutingStrategy;

pub(crate) async fn resolve_order(
    strategy: &Option<Box<dyn RoutingStrategy>>,
    messages: &Messages,
    metadata: &[Option<&ProviderMeta>],
) -> Result<Vec<usize>, ChatError> {
    match strategy {
        Some(strategy) => strategy
            .rank(messages, metadata)
            .await
            .map_err(|e| ChatError::Other(e.to_string())),
        None => Ok((0..metadata.len()).collect()),
    }
}

pub struct Router {
    pub(crate) providers: Vec<Box<dyn CompletionProvider>>,
    pub(crate) strategy: Option<Box<dyn RoutingStrategy>>,
    pub(crate) circuit_breaker: Option<CircuitBreaker>,
}

#[async_trait::async_trait]
impl CompletionProvider for Router {
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        let count = self.providers.len();
        if count == 0 {
            return Err(ChatFailure::from_err(ChatError::Other(
                "Router has no providers".to_string(),
            )));
        }

        let metadata: Vec<Option<&ProviderMeta>> =
            self.providers.iter().map(|p| p.metadata()).collect();
        let order = resolve_order(&self.strategy, messages, &metadata)
            .await
            .map_err(ChatFailure::from_err)?;

        let mut last_failure: Option<ChatFailure> = None;
        let mut tried_any = false;

        for idx in order {
            if let Some(cb) = &self.circuit_breaker
                && !cb.is_available(idx)
            {
                continue;
            }

            let provider = match self.providers.get_mut(idx) {
                Some(p) => p,
                None => {
                    return Err(ChatFailure::from_err(ChatError::Other(format!(
                        "Strategy returned out-of-range index {idx} for {count} providers"
                    ))));
                }
            };

            tried_any = true;

            match provider
                .complete(messages, tool_declarations, options, structured_output)
                .await
            {
                Ok(response) => {
                    if let Some(cb) = &mut self.circuit_breaker {
                        cb.record_success(idx);
                    }
                    return Ok(response);
                }
                Err(failure) => {
                    // Non-retryable errors (bad request, invalid response, …)
                    // won't fare better on another provider and shouldn't be
                    // masked behind a later failure — surface immediately. Only
                    // retryable failures fall through to the next provider.
                    if !failure.err.is_retryable() {
                        return Err(failure);
                    }
                    if let Some(cb) = &mut self.circuit_breaker {
                        cb.record_failure(idx);
                    }
                    last_failure = Some(failure);
                }
            }
        }

        // All circuits were open — try the one open the longest as a last resort
        if !tried_any
            && let Some(cb) = &self.circuit_breaker
            && let Some(idx) = cb.longest_open()
            && let Some(provider) = self.providers.get_mut(idx)
        {
            match provider
                .complete(messages, tool_declarations, options, structured_output)
                .await
            {
                Ok(response) => {
                    if let Some(cb) = &mut self.circuit_breaker {
                        cb.record_success(idx);
                    }
                    return Ok(response);
                }
                Err(failure) => {
                    if let Some(cb) = &mut self.circuit_breaker {
                        cb.record_failure(idx);
                    }
                    return Err(failure);
                }
            }
        }

        Err(last_failure.unwrap_or_else(|| {
            ChatFailure::from_err(ChatError::Other("All providers exhausted".to_string()))
        }))
    }
}
