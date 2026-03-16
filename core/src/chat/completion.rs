use schemars::JsonSchema;

use crate::chat::Chat;
use crate::types::response::StructuredResponse;
use crate::{
    chat::state::{Structured, Unstructured},
    error::{ChatError, ChatFailure},
    traits::CompletionProvider,
    types::{
        callback::CallbackRetryContext,
        messages::{Messages, content::Content, parts::PartEnum},
        metadata::Metadata,
        response::ChatResponse,
    },
};
use serde::de::DeserializeOwned;

impl<CP: CompletionProvider> Chat<CP, Unstructured> {
    pub async fn complete(&mut self, messages: &mut Messages) -> Result<ChatResponse, ChatFailure> {
        self.execute_with_retries(messages, |response| {
            Ok(ChatResponse {
                content: response.content.clone(),
                metadata: response.metadata.clone(),
            })
        })
        .await
    }
}

impl<CP: CompletionProvider, T> Chat<CP, Structured<T>>
where
    T: DeserializeOwned + JsonSchema,
{
    pub async fn complete(
        &mut self,
        messages: &mut Messages,
    ) -> Result<StructuredResponse<T>, ChatFailure> {
        self.execute_with_retries(messages, |response| {
            let value = extract_structured_candidate(&response.content).ok_or_else(|| {
                ChatError::InvalidResponse(
                    "Response did not contain valid structured output".into(),
                )
            })?;
            serde_json::from_value::<T>(value.clone())
                .map(|content| StructuredResponse {
                    content,
                    metadata: response.metadata.clone(),
                })
                .map_err(|err| {
                    ChatError::InvalidResponse(format!(
                        "Failed to parse structured output: {}",
                        err
                    ))
                })
        })
        .await
    }
}

impl<CP: CompletionProvider, Output> Chat<CP, Output> {
    async fn call_loop(&mut self, messages: &mut Messages) -> Result<ChatResponse, ChatFailure> {
        let mut last_metadata: Option<Metadata> = None;

        for _ in 0..self.max_steps.unwrap_or(1) {
            let response = self
                .model
                .complete(
                    messages,
                    self.tools.as_ref(),
                    self.model_options.as_ref(),
                    self.output_shape.as_ref(),
                )
                .await?;

            if let Some(metadata) = response.metadata.clone() {
                match &mut last_metadata {
                    Some(existing) => {
                        existing.extend(&metadata);
                    }
                    None => {
                        last_metadata = Some(metadata);
                    }
                }
            }

            messages.push(response.content.clone());

            if let Ok(frs) = self.tool_call(&response.content).await
                && !frs.is_empty()
            {
                let mut tool_message = Content::default();
                tool_message.parts.extend(frs);
                messages.push(tool_message);
                continue;
            }

            match response.content.parts.last() {
                Some(res) => match res {
                    PartEnum::Text(_) | PartEnum::Structured(_) => {
                        return Ok(ChatResponse {
                            metadata: last_metadata,
                            content: response.content,
                        });
                    }
                    PartEnum::Reasoning(_) => {
                        continue;
                    }
                    _ => {}
                },
                None => {
                    return Err(ChatFailure {
                        err: ChatError::InvalidResponse(
                            "Response did not generate any parts".to_string(),
                        ),
                        metadata: last_metadata,
                    });
                }
            };
        }

        Err(ChatFailure {
            err: ChatError::MaxStepsExceeded,
            metadata: last_metadata,
        })
    }

    async fn execute_with_retries<F, R>(
        &mut self,
        messages: &mut Messages,
        mut processor: F,
    ) -> Result<R, ChatFailure>
    where
        F: FnMut(&ChatResponse) -> Result<R, ChatError>,
    {
        let max_retries = self.max_retries.unwrap_or(1);
        let mut last_err: Option<ChatError> = None;
        let mut last_metadata: Option<Metadata> = None;

        if let Some(strategy) = self.before_strategy.as_mut() {
            strategy(messages, last_metadata.as_ref()).await;
        }

        for idx in 0..max_retries {
            let original_len = messages.len();
            match self.call_loop(messages).await {
                Ok(response) => {
                    if let Some(metadata) = response.metadata.clone() {
                        match &mut last_metadata {
                            Some(existing) => {
                                existing.extend(&metadata);
                            }
                            None => {
                                last_metadata = Some(metadata);
                            }
                        }
                    }

                    match processor(&response) {
                        Ok(parsed_result) => {
                            if let Some(strategy) = self.after_strategy.as_mut() {
                                strategy(messages, last_metadata.as_ref()).await;
                            }
                            return Ok(parsed_result);
                        }
                        Err(err) => {
                            last_err = Some(err.clone());
                            if idx + 1 < max_retries {
                                let ctx = CallbackRetryContext {
                                    idx,
                                    failure: ChatFailure {
                                        err,
                                        metadata: last_metadata.clone(),
                                    },
                                };
                                if let Some(strategy) = self.retry_strategy.as_mut() {
                                    strategy(messages, last_metadata.as_ref(), ctx).await;
                                }
                            }
                        }
                    }
                }
                Err(failure) => {
                    if let Some(metadata) = failure.metadata.clone() {
                        match &mut last_metadata {
                            Some(existing) => {
                                existing.extend(&metadata);
                            }
                            None => {
                                last_metadata = Some(metadata);
                            }
                        }
                    }

                    last_err = Some(failure.err.clone());

                    if !failure.err.is_retryable() {
                        break;
                    }

                    if idx + 1 < max_retries {
                        let ctx = CallbackRetryContext { idx, failure };
                        if let Some(strategy) = self.retry_strategy.as_mut() {
                            strategy(messages, last_metadata.as_ref(), ctx).await;
                        }
                    }
                }
            }

            messages.0.truncate(original_len);
        }

        Err(ChatFailure {
            metadata: last_metadata,
            err: last_err.unwrap_or(ChatError::RateLimited),
        })
    }
}

fn extract_structured_candidate(content: &Content) -> Option<serde_json::Value> {
    let last = content.parts.last()?;

    match last {
        PartEnum::Structured(v) => Some(v.clone()),
        PartEnum::Text(t) => serde_json::from_str::<serde_json::Value>(t.as_str()).ok(),
        _ => None,
    }
}
