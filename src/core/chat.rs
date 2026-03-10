use schemars::JsonSchema;

use serde::de::DeserializeOwned;
use tools_rs::ToolCollection;

use crate::{
    callback::{CallbackRetryContext, CallbackStrategy, RetryStrategy},
    core::{
        lib::{ChatError, ChatOptions, ChatProvider},
        messages::{
            Messages,
            content::Content,
            parts::{PartEnum, Parts},
        },
    },
    lib::{ChatFailure, ChatResponse, EmbeddingsResponse},
    metadata::Metadata,
};

pub struct Unstructured;
pub struct Structured<T>(std::marker::PhantomData<T>);
pub struct Streamed;

#[derive(Default)]
pub struct Chat<CP: ChatProvider, Output = Unstructured> {
    pub(crate) model: CP,
    pub(crate) output_shape: Option<schemars::Schema>,
    pub(crate) model_options: Option<ChatOptions>,
    pub(crate) max_steps: Option<u16>,
    pub(crate) max_retries: Option<u16>,
    pub(crate) retry_strategy: Option<RetryStrategy>,
    pub(crate) before_strategy: Option<CallbackStrategy>,
    pub(crate) after_strategy: Option<CallbackStrategy>,
    pub(crate) tools: Option<ToolCollection>,
    pub(crate) _output: std::marker::PhantomData<Output>,
}

impl<CP: ChatProvider> Chat<CP, Unstructured> {
    pub async fn complete(&mut self, messages: &mut Messages) -> Result<ChatResponse, ChatFailure> {
        self.execute_with_retries(messages, |response| {
            Ok(ChatResponse {
                content: response.content.clone(),
                metadata: response.metadata.clone(),
            })
        })
        .await
    }

    pub async fn embed(
        &mut self,
        messages: &mut Messages,
    ) -> Result<EmbeddingsResponse, ChatFailure> {
        if self.max_steps.is_some() {
            println!("Warning, embeddings is a one shot call, it does not implement steps")
        }

        if let Some(strategy) = self.before_strategy.as_mut() {
            strategy(messages, None).await;
        }
        let response = self.model.complete(messages, None, None, None).await?;

        let metadata = response.metadata;
        let embeddings_part = response.content.parts.last().ok_or_else(|| ChatFailure {
            err: ChatError::InvalidResponse("No parts in response".to_string()),
            metadata: metadata.clone(),
        })?;

        match embeddings_part {
            PartEnum::Embeddings(embeddings) => {
                if let Some(strategy) = self.after_strategy.as_mut() {
                    strategy(messages, metadata.as_ref()).await;
                }

                Ok(EmbeddingsResponse {
                    metadata,
                    embeddings: embeddings.clone(),
                })
            }
            _ => {
                let failure = ChatFailure {
                    err: ChatError::InvalidResponse("Response was not embeddings".to_string()),
                    metadata: metadata.clone(),
                };

                let ctx = CallbackRetryContext {
                    idx: 0,
                    failure: failure.clone(),
                };

                if let Some(strategy) = self.retry_strategy.as_mut() {
                    strategy(messages, metadata.as_ref(), ctx).await;
                }

                Err(failure)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct StructuredResponse<T: DeserializeOwned + JsonSchema> {
    pub content: T,
    pub metadata: Option<Metadata>,
}

impl<CP: ChatProvider, T> Chat<CP, Structured<T>>
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
                    metadata: None,
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

impl<CP: ChatProvider, Output> Chat<CP, Output> {
    pub(crate) async fn tool_call(&self, content: &Content) -> Result<Parts, ChatError> {
        let mut frs: Parts = Parts::default();
        for fc in content.parts.function_calls() {
            frs.push(PartEnum::from_function_response(
                self.tools
                    .clone()
                    .ok_or(ChatError::InvalidResponse(
                        "Attempted to call tool but no tool collection has been set.".to_string(),
                    ))?
                    .call(fc.clone())
                    .await
                    .map_err(|_err| ChatError::InvalidResponse("Tools error".to_string()))?,
            ));
        }
        Ok(frs)
    }

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
            err: ChatError::RateLimited,
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
