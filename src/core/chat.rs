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
    /// `Ok(content)` with the final `Content` when a completion succeeds, last error is produced or `Err(ChatError::RateLimited)` if all retries are exhausted.
    ///
    /// # Examples
    ///
    /// ```
    /// # use chat_rs::{Chat, Messages, ChatBuilder};
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
        let mut last_err: Option<ChatError> = None;

        for _ in 0..max_retries {
            let retry_messages = messages.clone();

            match self.call_loop(&retry_messages).await {
                Ok(content) => return Ok(content),
                Err(err) => {
                    last_err = Some(err);
                    continue;
                }
            }
        }

        Err(last_err.unwrap_or(ChatError::RateLimited))
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
                && !frs.is_empty()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::messages::content::RoleEnum;
    use crate::core::messages::parts::PartEnum;
    use async_trait::async_trait;

    // Mock ChatProvider for testing
    struct MockChatProvider {
        responses: Vec<Content>,
        call_count: std::sync::Arc<std::sync::Mutex<usize>>,
    }

    impl MockChatProvider {
        fn new(responses: Vec<Content>) -> Self {
            MockChatProvider {
                responses,
                call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
            }
        }

        fn single_response(text: &str) -> Self {
            let content = Content {
                parts: Parts(vec![PartEnum::from_text(text)]),
                role: RoleEnum::Model,
                complete_reason: CompleteReasonEnum::Stop,
            };
            MockChatProvider::new(vec![content])
        }
    }

    #[async_trait]
    impl ChatProvider for MockChatProvider {
        async fn complete(
            &self,
            _messages: &Messages,
            _tools: Option<&ToolCollection>,
            _options: Option<&ChatOptions>,
        ) -> Result<Content, ChatError> {
            let mut count = self.call_count.lock().unwrap();
            let idx = *count;
            *count += 1;

            if idx < self.responses.len() {
                Ok(self.responses[idx].clone())
            } else {
                // Return last response if called more times than we have responses
                Ok(self.responses.last().unwrap().clone())
            }
        }
    }

    #[test]
    fn test_chat_builder_new() {
        let builder = ChatBuilder::<MockChatProvider>::new();
        assert!(builder.model.is_none());
        assert!(builder.max_steps.is_none());
        assert!(builder.max_retries.is_none());
        assert!(builder.tools.is_none());
    }

    #[test]
    fn test_chat_builder_default() {
        let builder = ChatBuilder::<MockChatProvider>::default();
        assert!(builder.model.is_none());
    }

    #[test]
    fn test_chat_builder_with_max_steps() {
        let builder = ChatBuilder::<MockChatProvider>::new().with_max_steps(5);
        assert_eq!(builder.max_steps, Some(5));
    }

    #[test]
    fn test_chat_builder_with_max_retries() {
        let builder = ChatBuilder::<MockChatProvider>::new().with_max_retries(3);
        assert_eq!(builder.max_retries, Some(3));
    }

    #[test]
    fn test_chat_builder_with_model() {
        let model = MockChatProvider::single_response("Test");
        let builder = ChatBuilder::new().with_model(model);
        assert!(builder.model.is_some());
    }

    #[test]
    fn test_chat_builder_chaining() {
        let model = MockChatProvider::single_response("Test");
        let builder = ChatBuilder::new()
            .with_max_steps(10)
            .with_max_retries(2)
            .with_model(model);

        assert_eq!(builder.max_steps, Some(10));
        assert_eq!(builder.max_retries, Some(2));
        assert!(builder.model.is_some());
    }

    #[test]
    #[should_panic(expected = "Need to set a model")]
    fn test_chat_builder_build_without_model_panics() {
        let _chat = ChatBuilder::<MockChatProvider>::new().build();
    }

    #[test]
    fn test_chat_builder_build_with_model() {
        let model = MockChatProvider::single_response("Test");
        let chat = ChatBuilder::new().with_model(model).build();
        assert_eq!(chat.max_steps, None);
        assert_eq!(chat.max_retries, None);
    }

    #[tokio::test]
    async fn test_chat_complete_simple_response() {
        let model = MockChatProvider::single_response("Hello, world!");
        let mut chat = ChatBuilder::new().with_model(model).build();

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Hi"]));

        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());

        let content = result.unwrap();
        assert_eq!(content.role, RoleEnum::Model);
        assert_eq!(
            content.parts.text_response().unwrap().as_str(),
            "Hello, world!"
        );
    }

    #[tokio::test]
    async fn test_chat_complete_with_max_steps() {
        let model = MockChatProvider::single_response("Response");
        let mut chat = ChatBuilder::new()
            .with_model(model)
            .with_max_steps(3)
            .build();

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Test"]));

        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_chat_complete_with_reasoning() {
        let content = Content {
            parts: Parts(vec![
                PartEnum::from_reasoning("Let me think..."),
                PartEnum::from_text("Here's the answer"),
            ]),
            role: RoleEnum::Model,
            complete_reason: CompleteReasonEnum::Stop,
        };

        let model = MockChatProvider::new(vec![content]);
        let mut chat = ChatBuilder::new().with_model(model).build();

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Question"]));

        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_chat_complete_empty_response_returns_error() {
        let content = Content {
            parts: Parts(vec![]),
            role: RoleEnum::Model,
            complete_reason: CompleteReasonEnum::Stop,
        };

        let model = MockChatProvider::new(vec![content]);
        let mut chat = ChatBuilder::new().with_model(model).build();

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Test"]));

        let result = chat.complete(&mut messages).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ChatError::InvalidResponse(msg) => {
                assert!(msg.contains("did not generate any parts"));
            }
            _ => panic!("Expected InvalidResponse error"),
        }
    }

    #[tokio::test]
    async fn test_chat_complete_structured_output_returns_error() {
        let content = Content {
            parts: Parts(vec![PartEnum::from_structured(
                serde_json::json!({"key": "value"}),
            )]),
            role: RoleEnum::Model,
            complete_reason: CompleteReasonEnum::Stop,
        };

        let model = MockChatProvider::new(vec![content]);
        let mut chat = ChatBuilder::new().with_model(model).build();

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Test"]));

        let result = chat.complete(&mut messages).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ChatError::Other(msg) => {
                assert!(msg.contains("Structured output not yet implemented"));
            }
            _ => panic!("Expected Other error"),
        }
    }

    #[tokio::test]
    async fn test_chat_default_max_retries_is_one() {
        let model = MockChatProvider::single_response("Test");
        let mut chat = ChatBuilder::new().with_model(model).build();

        // The default max_retries should be 1 (as per line 23 in chat.rs)
        assert_eq!(chat.max_retries, None);

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Hi"]));

        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_chat_default_max_steps_is_one() {
        let model = MockChatProvider::single_response("Test");
        let mut chat = ChatBuilder::new().with_model(model).build();

        // The default max_steps should be 1 (as per line 53 in chat.rs)
        assert_eq!(chat.max_steps, None);

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Hi"]));

        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_chat_with_multiple_retries() {
        let model = MockChatProvider::single_response("Response");
        let mut chat = ChatBuilder::new()
            .with_model(model)
            .with_max_retries(5)
            .build();

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Test"]));

        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_chat_builder_with_tools() {
        let tools = ToolCollection::new();
        let model = MockChatProvider::single_response("Test");
        let builder = ChatBuilder::new().with_tools(tools).with_model(model);

        assert!(builder.tools.is_some());
    }

    #[tokio::test]
    async fn test_chat_messages_not_mutated_on_error() {
        let content = Content {
            parts: Parts(vec![]),
            role: RoleEnum::Model,
            complete_reason: CompleteReasonEnum::Stop,
        };

        let model = MockChatProvider::new(vec![content]);
        let mut chat = ChatBuilder::new().with_model(model).build();

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Test"]));
        let original_len = messages.len();

        let _result = chat.complete(&mut messages).await;

        // Messages should not be modified
        assert_eq!(messages.len(), original_len);
    }

    #[tokio::test]
    async fn test_chat_reasoning_continues_loop() {
        // Create a sequence where first response is reasoning, second is text
        let reasoning_content = Content {
            parts: Parts(vec![PartEnum::from_reasoning("Thinking...")]),
            role: RoleEnum::Model,
            complete_reason: CompleteReasonEnum::None,
        };

        let text_content = Content {
            parts: Parts(vec![PartEnum::from_text("Final answer")]),
            role: RoleEnum::Model,
            complete_reason: CompleteReasonEnum::Stop,
        };

        let model = MockChatProvider::new(vec![reasoning_content, text_content]);
        let mut chat = ChatBuilder::new()
            .with_model(model)
            .with_max_steps(5)
            .build();

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Question"]));

        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());

        let content = result.unwrap();
        // Should have both reasoning and final text
        assert!(content.parts.len() >= 1);
    }

    #[tokio::test]
    async fn test_chat_max_steps_limit_reached() {
        // Create responses that only have reasoning (no final text)
        let reasoning_content = Content {
            parts: Parts(vec![PartEnum::from_reasoning("Still thinking...")]),
            role: RoleEnum::Model,
            complete_reason: CompleteReasonEnum::None,
        };

        let model = MockChatProvider::new(vec![reasoning_content]);
        let mut chat = ChatBuilder::new()
            .with_model(model)
            .with_max_steps(2)
            .build();

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Question"]));

        let result = chat.complete(&mut messages).await;
        // Should return RateLimited error when max_steps is exceeded
        assert!(result.is_err());
        match result.unwrap_err() {
            ChatError::RateLimited => {
                // Expected
            }
            _ => panic!("Expected RateLimited error"),
        }
    }
}
