use crate::{
    chat::{Chat, state::Embedded},
    error::{ChatError, ChatFailure},
    traits::EmbeddingsProvider,
    types::{
        callback::CallbackRetryContext, messages::Messages, metadata::Metadata,
        response::EmbeddingsResponse,
    },
};

impl<CP: EmbeddingsProvider> Chat<CP, Embedded> {
    pub async fn embed(
        &mut self,
        messages: &mut Messages,
    ) -> Result<EmbeddingsResponse, ChatFailure> {
        if let Some(strategy) = self.before_strategy.as_mut() {
            strategy(messages, None).await;
        }
        let max_retries = self.max_retries.unwrap_or(1);
        let mut last_metadata: Option<Metadata> = None;

        for idx in 0..max_retries {
            match self.model.embed(messages).await {
                Ok(res) => {
                    return Ok(res);
                }
                Err(failure) => {
                    if let Some(metadata) = failure.metadata.as_ref() {
                        match &mut last_metadata {
                            Some(existing) => {
                                existing.extend(metadata);
                            }
                            None => {
                                last_metadata = Some(metadata.clone());
                            }
                        }
                    }

                    let ctx = CallbackRetryContext { idx, failure };

                    if let Some(strategy) = self.retry_strategy.as_mut() {
                        strategy(messages, last_metadata.as_ref(), ctx).await
                    }

                    continue;
                }
            }
        }

        Err(ChatFailure {
            err: ChatError::RateLimited,
            metadata: last_metadata,
        })
    }
}
