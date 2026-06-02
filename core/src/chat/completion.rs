use schemars::JsonSchema;

use crate::chat::Chat;
use crate::types::response::{ChatOutcome, PauseReason, StructuredResponse};
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
    /// Run the chat loop until the model completes, max_steps is reached,
    /// or a tool call strategy pauses execution (pending approval,
    /// scheduled, etc). Callers handle `ChatOutcome::Paused` by mutating
    /// pending tool statuses and invoking [`Chat::resume`].
    pub async fn complete(
        &mut self,
        messages: &mut Messages,
    ) -> Result<ChatOutcome<ChatResponse>, ChatFailure> {
        self.execute_with_retries(messages, |response| {
            Ok(ChatResponse {
                content: response.content.clone(),
                metadata: response.metadata.clone(),
            })
        })
        .await
    }

    /// Resume a loop that previously returned `ChatOutcome::Paused`. The
    /// caller is expected to have resolved at least one pending tool
    /// (typically by calling `tool.approve(...)` or `tool.reject(...)`
    /// on each) before calling resume.
    pub async fn resume(
        &mut self,
        messages: &mut Messages,
    ) -> Result<ChatOutcome<ChatResponse>, ChatFailure> {
        self.resume_with(messages, |response| {
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
    ) -> Result<ChatOutcome<StructuredResponse<T>>, ChatFailure> {
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

/// Internal loop result: either the model reached a terminal text/structured
/// response, or a tool-call strategy paused us.
enum LoopStep {
    Complete(ChatResponse),
    Paused(PauseReason, Option<Metadata>),
}

impl<CP: CompletionProvider, Output> Chat<CP, Output> {
    async fn call_loop(&mut self, messages: &mut Messages) -> Result<LoopStep, ChatFailure> {
        let mut last_metadata: Option<Metadata> = None;

        if let Some(last) = messages.0.last_mut() {
            let pre = self.tool_call(last).await.map_err(|err| ChatFailure {
                err,
                metadata: None,
            })?;
            if let Some(reason) = pre.pause {
                return Ok(LoopStep::Paused(reason, last_metadata));
            }
        }

        for _ in 0..self.max_steps.unwrap_or(1) {
            let decls = crate::chat::tool_declarations_from(&self.scoped_collections);
            let decls_dyn = decls
                .as_ref()
                .map(|d| d as &dyn crate::types::tools::ToolDeclarations);
            let response = self
                .model
                .complete(
                    messages,
                    decls_dyn,
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

            let pass = match messages.0.last_mut() {
                Some(last) => self.tool_call(last).await.map_err(|err| ChatFailure {
                    err,
                    metadata: last_metadata.clone(),
                })?,
                None => crate::chat::ToolCallPass::default(),
            };

            if let Some(reason) = pass.pause {
                return Ok(LoopStep::Paused(reason, last_metadata));
            }
            if pass.executed {
                continue;
            }

            match response.content.parts.last() {
                Some(res) => match res {
                    PartEnum::Text(_) | PartEnum::Structured(_) => {
                        return Ok(LoopStep::Complete(ChatResponse {
                            metadata: last_metadata,
                            content: response.content,
                        }));
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
    ) -> Result<ChatOutcome<R>, ChatFailure>
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
                Ok(LoopStep::Paused(reason, _metadata)) => {
                    return Ok(ChatOutcome::Paused { reason });
                }
                Ok(LoopStep::Complete(response)) => {
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
                            return Ok(ChatOutcome::Complete(parsed_result));
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

    /// Resume helper for structured / unstructured variants. Does not
    /// run retries — resume is always a continuation of a prior attempt,
    /// not a new one.
    async fn resume_with<F, R>(
        &mut self,
        messages: &mut Messages,
        mut processor: F,
    ) -> Result<ChatOutcome<R>, ChatFailure>
    where
        F: FnMut(&ChatResponse) -> Result<R, ChatError>,
    {
        match self.call_loop(messages).await? {
            LoopStep::Paused(reason, _) => Ok(ChatOutcome::Paused { reason }),
            LoopStep::Complete(response) => match processor(&response) {
                Ok(parsed) => Ok(ChatOutcome::Complete(parsed)),
                Err(err) => Err(ChatFailure {
                    err,
                    metadata: response.metadata,
                }),
            },
        }
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
