use tools_rs::ToolCollection;

use crate::core::{
    lib::{ChatError, ChatOptions, ChatProvider},
    messages::{
        Messages,
        content::{CompleteReasonEnum, Content},
        parts::{PartEnum, Parts},
    },
};

#[derive(Default)]
pub struct Chat<CP: ChatProvider> {
    model: CP,
    model_options: Option<ChatOptions>,
    max_steps: Option<i16>,
    max_retries: Option<i16>,
    tools: Option<ToolCollection>,
}

impl<CP: ChatProvider> Chat<CP> {
    /// Perform a chat completion using the configured model, retrying up to the builder-configured number of attempts.
    ///
    /// # Returns
    ///
    /// `Ok(content)` with the final `Content` when a completion succeeds, `Err(ChatError::RateLimited)` if all retries are exhausted.
    ///
    /// # Examples
    ///
    /// ```
    /// # use your_crate::{Chat, Messages, ChatBuilder};
    /// # async fn example() {
    /// let mut chat = ChatBuilder::new()
    ///     .with_max_retries(2)
    ///     // .with_model(...) configure a model here
    ///     .build();
    /// let mut messages = Messages::new();
    /// // populate messages...
    /// let result = chat.complete(&mut messages).await;
    /// match result {
    ///     Ok(content) => println!("Got content: {:?}", content),
    ///     Err(e) => eprintln!("Completion failed: {:?}", e),
    /// }
    /// # }
    /// ```
    pub async fn complete(&mut self, messages: &mut Messages) -> Result<Content, ChatError> {
        let max_retries = self.max_retries.unwrap_or(1);
        for _ in 0..max_retries {
            let retry_messages = messages.clone();
            return match self.call_loop(&retry_messages).await {
                Ok(content) => Ok(content),
                Err(_) => continue,
            };
        }
        Err(ChatError::RateLimited)
    }

    /// Calls each function-call part in `content` using the chat's configured tool collection and
    /// returns the collected function responses as `Parts`.
    ///
    /// If no tool collection is configured, or a tool invocation fails, an `Err(ChatError::InvalidResponse)`
    /// is returned.
    ///
    /// # Examples
    ///
    /// ```
    /// // Assume `chat` is a Chat instance with tools configured and `content` contains function calls.
    /// // let parts = chat.tool_call(&content).await.unwrap();
    /// ```
    async fn tool_call(&self, content: &Content) -> Result<Parts, ChatError> {
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
                    .map_err(|_err| ChatError::InvalidResponse("Tools error: {}".to_string()))?,
            ));
        }
        Ok(frs)
    }

    /// Drives an iterative model completion loop, optionally invoking tools, until a terminal text part is produced or a stopping error occurs.
    ///
    /// This method clones the provided `messages` and repeatedly calls the configured chat model (up to `max_steps.unwrap_or(1)` iterations). After each model response it attempts tool calls and, if tools return parts, appends them to the response. The loop examines the response's last part:
    /// - If the last part is text, the response is returned as the final content.
    /// - If the last part is reasoning, that reasoning is converted into a part and appended so the loop can continue.
    /// - If the last part is structured, an error is returned since structured outputs are not implemented.
    /// If a response contains no parts, an `InvalidResponse` error is returned. If the loop completes without producing a terminal text part, a `RateLimited` error is returned. Errors from the underlying model completion are propagated.
    ///
    /// # Parameters
    ///
    /// - `messages`: the conversation history to drive the completion loop; this value is cloned and extended internally as the loop progresses.
    ///
    /// # Returns
    ///
    /// `Ok(Content)` with the final response whose last part is text, or `Err(ChatError)` if the model returns an error, the response is invalid or structured, or the loop exhausts allowed steps.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Construct a Chat instance and messages, then run the loop:
    /// // let mut chat: Chat<MyProvider> = /* configured chat */ ;
    /// // let messages = Messages::default();
    /// // let result = tokio::runtime::Runtime::new().unwrap().block_on(async {
    /// //     chat.call_loop(&messages).await
    /// // });
    /// // assert!(result.is_ok() || result.is_err());
    /// ```
    async fn call_loop(&mut self, messages: &Messages) -> Result<Content, ChatError> {
        let mut inner_messages = messages.clone();
        for _ in 0..self.max_steps.unwrap_or(1) {
            let mut response = self
                .model
                .complete(
                    &inner_messages,
                    self.tools.as_ref(),
                    self.model_options.as_ref(),
                )
                .await?;
            if let Ok(frs) = self.tool_call(&response).await
                && frs.length() > 0
            {
                response.parts.extend(frs);
            }

            match response.parts.last() {
                Some(res) => match res {
                    PartEnum::Text(_text) => return Ok(response),
                    PartEnum::Reasoning(reasoning) => {
                        response
                            .parts
                            .push(PartEnum::from_reasoning(reasoning.to_owned()));
                    }
                    PartEnum::Structured(_) => {
                        return Err(ChatError::Other(
                            "Structured output not yet implemented".to_string(),
                        ));
                    }
                    _ => {}
                },
                None => {
                    return Err(ChatError::InvalidResponse(
                        "Response did not generate any parts".to_string(),
                    ));
                }
            };

            inner_messages.push(response.clone());
            println!(
                "Inner Messages: {:#?}\n Response (should match last inner message): {:#?}",
                inner_messages, response
            );
            /*
                            return match response.complete_reason {
                                CompleteReasonEnum::Stop => Ok(response),
                                CompleteReasonEnum::MaxTokens => Err(ChatError::RateLimited),
                                CompleteReasonEnum::Recitation => Err(ChatError::Provider(
                                    "Content response was recited".to_string(),
                                )),
                                CompleteReasonEnum::ContentFilter => Err(ChatError::Provider(
                                    "Content response was filtered".to_string(),
                                )),
                                CompleteReasonEnum::ToolCall => {
                                    // Gemini doesn't have this.
                                    response.parts.extend(self.tool_call(&response).await?);
                                    continue;
                                }
                                CompleteReasonEnum::None => {
                                    // Default implementation for completion when no stopping reason is provided
                                }
                            };
            */
        }
        Err(ChatError::RateLimited)
    }
}

pub struct ChatBuilder<CP: ChatProvider> {
    model: Option<CP>,
    model_options: Option<ChatOptions>,
    max_steps: Option<i16>,
    max_retries: Option<i16>,
    tools: Option<ToolCollection>,
}

impl<CP: ChatProvider> ChatBuilder<CP> {
    pub fn new() -> Self {
        ChatBuilder {
            model: None,
            max_steps: None,
            max_retries: None,
            tools: None,
            model_options: None,
        }
    }

    pub fn with_max_steps(mut self, max_steps: i16) -> Self {
        self.max_steps = Some(max_steps);
        self
    }

    pub fn with_max_retries(mut self, max_retries: i16) -> Self {
        self.max_retries = Some(max_retries);
        self
    }

    pub fn with_tools(mut self, tools: ToolCollection) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn with_model(mut self, model: CP) -> Self {
        self.model = Some(model);
        self
    }

    /// Builds a Chat instance from this builder, consuming the builder.
    ///
    /// # Panics
    ///
    /// Panics if a model was not provided via `with_model`.
    ///
    /// # Examples
    ///
    /// ```
    /// // Provide a concrete type that implements `ChatProvider` for `CP`.
    /// // let model = MyChatModel::new(...);
    /// // let chat = ChatBuilder::new().with_model(model).build();
    /// ```
    pub fn build(self) -> Chat<CP> {
        Chat {
            model: self.model.expect("Need to set a model"),
            max_steps: self.max_steps,
            max_retries: self.max_retries,
            tools: self.tools,
            model_options: self.model_options,
        }
    }
}

impl<CP: ChatProvider> Default for ChatBuilder<CP> {
    /// Creates a ChatBuilder with default (unset) configuration.
    ///
    /// All optional builder fields are initialized to None; equivalent to calling
    /// `ChatBuilder::new()`.
    ///
    /// # Examples
    ///
    /// ```
    /// let _builder: ChatBuilder<()> = Default::default();
    /// ```
    fn default() -> Self {
        ChatBuilder::new()
    }
}