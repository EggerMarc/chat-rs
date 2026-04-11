use chat_core::error::{ChatError, ChatFailure};
use chat_core::traits::{ChatProvider, CompletionProvider, StreamProvider};
use chat_core::types::messages::Messages;
use chat_core::types::options::ChatOptions;
use chat_core::types::provider_meta::ProviderMeta;
use chat_core::types::response::{ChatResponse, StreamEvent};
use chat_core::types::tools::ToolDeclarations;
use futures::stream::BoxStream;

use crate::circuit_breaker::CircuitBreaker;
use crate::router::resolve_order;
use crate::strategy::RoutingStrategy;

pub struct StreamRouter {
    pub(crate) providers: Vec<Box<dyn ChatProvider>>,
    pub(crate) strategy: Option<Box<dyn RoutingStrategy>>,
    pub(crate) circuit_breaker: Option<CircuitBreaker>,
    pub(crate) last_used: Option<usize>,
}

#[async_trait::async_trait]
impl CompletionProvider for StreamRouter {
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
                "StreamRouter has no providers".to_string(),
            )));
        }

        let metadata: Vec<Option<&ProviderMeta>> =
            self.providers.iter().map(|p| p.metadata()).collect();
        let order = resolve_order(&self.strategy, messages, &metadata)
            .await
            .map_err(|e| ChatFailure::from_err(e))?;

        let mut last_failure: Option<ChatFailure> = None;
        let mut tried_any = false;

        for idx in order {
            if let Some(cb) = &self.circuit_breaker {
                if !cb.is_available(idx) {
                    continue;
                }
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
                    self.last_used = Some(idx);
                    return Ok(response);
                }
                Err(failure) => {
                    if let Some(cb) = &mut self.circuit_breaker {
                        cb.record_failure(idx);
                    }
                    last_failure = Some(failure);
                }
            }
        }

        if !tried_any {
            if let Some(cb) = &self.circuit_breaker {
                if let Some(idx) = cb.longest_open() {
                    if let Some(provider) = self.providers.get_mut(idx) {
                        match provider
                            .complete(messages, tool_declarations, options, structured_output)
                            .await
                        {
                            Ok(response) => {
                                if let Some(cb) = &mut self.circuit_breaker {
                                    cb.record_success(idx);
                                }
                                self.last_used = Some(idx);
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

#[async_trait::async_trait]
impl StreamProvider for StreamRouter {
    async fn stream(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError> {
        let count = self.providers.len();
        if count == 0 {
            return Err(ChatError::Other(
                "StreamRouter has no providers".to_string(),
            ));
        }

        let metadata: Vec<Option<&ProviderMeta>> =
            self.providers.iter().map(|p| p.metadata()).collect();
        let order = resolve_order(&self.strategy, messages, &metadata).await?;

        let mut last_error: Option<ChatError> = None;
        let mut tried_any = false;

        for idx in order {
            if let Some(cb) = &self.circuit_breaker {
                if !cb.is_available(idx) {
                    continue;
                }
            }

            let provider = match self.providers.get_mut(idx) {
                Some(p) => p,
                None => {
                    return Err(ChatError::Other(format!(
                        "Strategy returned out-of-range index {idx} for {count} providers"
                    )));
                }
            };

            tried_any = true;

            match provider.stream(messages, tool_declarations, options).await {
                Ok(stream) => {
                    if let Some(cb) = &mut self.circuit_breaker {
                        cb.record_success(idx);
                    }
                    self.last_used = Some(idx);
                    return Ok(stream);
                }
                Err(err) => {
                    if let Some(cb) = &mut self.circuit_breaker {
                        cb.record_failure(idx);
                    }
                    last_error = Some(err);
                }
            }
        }

        if !tried_any {
            if let Some(cb) = &self.circuit_breaker {
                if let Some(idx) = cb.longest_open() {
                    if let Some(provider) = self.providers.get_mut(idx) {
                        match provider.stream(messages, tool_declarations, options).await {
                            Ok(stream) => {
                                if let Some(cb) = &mut self.circuit_breaker {
                                    cb.record_success(idx);
                                }
                                self.last_used = Some(idx);
                                return Ok(stream);
                            }
                            Err(err) => {
                                if let Some(cb) = &mut self.circuit_breaker {
                                    cb.record_failure(idx);
                                }
                                return Err(err);
                            }
                        }
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            ChatError::Other("All providers exhausted".to_string())
        }))
    }

    fn on_stream_done(&mut self, response: &ChatResponse) {
        if let Some(idx) = self.last_used {
            if let Some(provider) = self.providers.get_mut(idx) {
                provider.on_stream_done(response);
            }
        }
    }
}
