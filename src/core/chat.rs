use schemars::JsonSchema;
use tools_rs::ToolCollection;

use crate::{
    core::{
        lib::{ChatError, ChatOptions, ChatProvider},
        messages::{
            Messages,
            content::Content,
            parts::{PartEnum, Parts},
        },
    },
    lib::{ChatFailure, ChatResponse},
    metadata::Metadata,
};

#[derive(Default)]
pub struct Chat<CP: ChatProvider> {
    model: CP,
    output_shape: Option<schemars::Schema>,
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
    pub async fn complete(&mut self, messages: &mut Messages) -> Result<ChatResponse, ChatFailure> {
        let max_retries = self.max_retries.unwrap_or(1);

        let mut last_err: Option<ChatError> = None;
        let mut last_metadata: Option<Metadata> = None;

        for _ in 0..max_retries {
            let retry_messages = messages.clone();

            match self.call_loop(&retry_messages).await {
                Ok(response) => {
                    if let Some(metadata) = response.metadata {
                        match &mut last_metadata {
                            Some(existing) => {
                                existing.extend(&metadata);
                            }
                            None => {
                                last_metadata = Some(metadata);
                            }
                        }
                    }

                    return Ok(ChatResponse {
                        content: response.content,
                        metadata: last_metadata,
                    });
                }

                Err(err) => {
                    if let Some(metadata) = err.metadata {
                        match &mut last_metadata {
                            Some(existing) => {
                                existing.extend(&metadata);
                            }
                            None => {
                                last_metadata = Some(metadata);
                            }
                        }
                    }

                    last_err = Some(err.err);
                }
            }
        }

        Err(ChatFailure {
            metadata: last_metadata,
            err: last_err.unwrap_or(ChatError::RateLimited),
        })
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

    /// Runs the model completion loop until a terminal response is produced or the allowed steps are exhausted.
    ///
    /// The provided `messages` are cloned and used as the conversation history for repeated model completions. After each model response, configured tools (if any) are invoked and their parts appended. Termination behavior:
    /// - If the final part is text, that `Content` is returned.
    /// - If the final part is structured, that `Content` is returned.
    /// - If the final part is reasoning, the reasoning is appended and the loop continues.
    /// - If a response contains no parts, an `InvalidResponse` error is returned.
    /// If the loop finishes without producing a terminal text or structured part, a `RateLimited` error is returned. Errors from the underlying model or tools are propagated.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use tokio;
    /// # async fn _example() {
    /// // let mut chat: Chat<MyProvider> = /* configured chat */ ;
    /// // let messages = Messages::default();
    /// // let result = chat.call_loop(&messages).await;
    /// // assert!(result.is_ok() || result.is_err());
    /// # }
    /// ```
    async fn call_loop(&mut self, messages: &Messages) -> Result<ChatResponse, ChatFailure> {
        let mut inner_messages = messages.clone();
        let mut last_metadata: Option<Metadata> = None;
        for _ in 0..self.max_steps.unwrap_or(1) {
            let mut response = self
                .model
                .complete(
                    &inner_messages,
                    self.tools.as_ref(),
                    self.model_options.as_ref(),
                    self.output_shape.as_ref(),
                )
                .await?;

            if let Some(metadata) = response.metadata {
                match &mut last_metadata {
                    Some(existing) => {
                        existing.extend(&metadata);
                    }
                    None => {
                        last_metadata = Some(metadata);
                    }
                }
            }

            if let Ok(frs) = self.tool_call(&response.content).await
                && !frs.is_empty()
            {
                response.content.parts.extend(frs);
            }

            match response.content.parts.last() {
                Some(res) => match res {
                    PartEnum::Text(_text) => return Ok(response),
                    PartEnum::Reasoning(reasoning) => {
                        response
                            .content
                            .parts
                            .push(PartEnum::from_reasoning(reasoning.to_owned()));
                    }
                    PartEnum::Structured(_structured) => {
                        return Ok(response);
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

            inner_messages.push(response.content.clone());
        }
        Err(ChatFailure {
            err: ChatError::RateLimited,
            metadata: last_metadata,
        })
    }
}

pub struct ChatBuilder<CP: ChatProvider> {
    model: Option<CP>,
    output_shape: Option<schemars::Schema>,
    model_options: Option<ChatOptions>,
    max_steps: Option<i16>,
    max_retries: Option<i16>,
    tools: Option<ToolCollection>,
}

impl<CP: ChatProvider> ChatBuilder<CP> {
    /// Create a new ChatBuilder with all configuration fields unset.
    ///
    /// # Examples
    ///
    /// ```
    /// // Type parameter must be provided so the builder knows the provider type.
    /// let _builder = ChatBuilder::<crate::MockChatProvider>::new();
    /// ```
    pub fn new() -> Self {
        ChatBuilder {
            model: None,
            max_steps: None,
            max_retries: None,
            output_shape: None,
            tools: None,
            model_options: None,
        }
    }

    /// Configure the builder to expect structured JSON output shaped like `S`.
    ///
    /// This consumes the builder and returns a new `ChatBuilder` with `output_shape` set to the schemars
    /// schema generated for `S`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use schemars::JsonSchema;
    ///
    /// #[derive(JsonSchema)]
    /// struct Out { pub a: String }
    ///
    /// let b = ChatBuilder::<()>::new().with_structured_output::<Out>();
    /// assert!(b.output_shape.is_some());
    /// ```
    pub fn with_structured_output<S>(self) -> ChatBuilder<CP>
    where
        S: JsonSchema + Send + Sync,
    {
        let shape = schemars::schema_for!(S);

        ChatBuilder {
            model: self.model,
            max_steps: self.max_steps,
            max_retries: self.max_retries,
            output_shape: Some(shape),
            tools: self.tools,
            model_options: self.model_options,
        }
    }

    /// Sets the maximum number of iterations the chat loop will perform when running `call_loop`.
    ///
    /// Returns the builder with `max_steps` set to the provided value.
    ///
    /// # Examples
    ///
    /// ```
    /// let builder = ChatBuilder::new().with_max_steps(3);
    /// assert_eq!(builder.max_steps, Some(3));
    /// ```
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

    /// Creates a Chat from this builder, consuming the builder.
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
            output_shape: self.output_shape,
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

/// Extracts a JSON candidate from the last part of `content`.
///
/// If the last part is `PartEnum::Structured`, returns its contained `serde_json::Value`.
/// If the last part is `PartEnum::Text` and the text parses as JSON, returns the parsed `Value`.
/// Returns `None` if there is no last part, the last part is neither `Structured` nor parsable `Text`.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// let c = Content { parts: vec![PartEnum::Text("{\"a\":1}".into())], Content::default()};
/// assert_eq!(extract_structured_candidate(&c), Some(json!({"a":1})));
///
/// let c2 = Content { parts: vec![PartEnum::Structured(json!({"b":2}))] };
/// assert_eq!(extract_structured_candidate(&c2), Some(json!({"b":2})));
///
/// let empty = Content { parts: vec![] };
/// assert_eq!(extract_structured_candidate(&empty), None);
/// ```
fn extract_structured_candidate(content: &Content) -> Option<serde_json::Value> {
    let last = content.parts.last()?;

    match last {
        PartEnum::Structured(v) => Some(v.clone()),
        PartEnum::Text(t) => serde_json::from_str::<serde_json::Value>(t.as_str()).ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::messages::content::RoleEnum;
    use crate::core::messages::parts::PartEnum;
    use crate::messages::content::CompleteReasonEnum;
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
                ..Default::default()
            };
            MockChatProvider::new(vec![content])
        }
    }

    #[async_trait]
    impl ChatProvider for MockChatProvider {
        /// Returns the next preconfigured `Content` response and advances the provider's internal call counter.
        ///
        /// If called more times than there are configured responses, this function returns the last response repeatedly.
        ///
        /// # Examples
        ///
        /// ```
        /// // Construct a mock provider with two prepared responses.
        /// let provider = MockChatProvider::new(vec![content_a.clone(), content_b.clone()]);
        ///
        /// // First call returns the first response.
        /// let first = tokio::runtime::Runtime::new().unwrap()
        ///     .block_on(provider.complete(&Messages::default(), None, None, None))
        ///     .unwrap();
        /// assert_eq!(first, content_a);
        ///
        /// // Second call returns the second response.
        /// let second = tokio::runtime::Runtime::new().unwrap()
        ///     .block_on(provider.complete(&Messages::default(), None, None, None))
        ///     .unwrap();
        /// assert_eq!(second, content_b);
        ///
        /// // Further calls return the last response repeatedly.
        /// let third = tokio::runtime::Runtime::new().unwrap()
        ///     .block_on(provider.complete(&Messages::default(), None, None, None))
        ///     .unwrap();
        /// assert_eq!(third, content_b);
        /// ```
        async fn complete(
            &self,
            _messages: &Messages,
            _tools: Option<&ToolCollection>,
            _options: Option<&ChatOptions>,
            _schema: Option<&schemars::Schema>,
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
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
            ..Default::default()
        };

        let text_content = Content {
            parts: Parts(vec![PartEnum::from_text("Final answer")]),
            role: RoleEnum::Model,
            complete_reason: CompleteReasonEnum::Stop,
            ..Default::default()
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
        assert!(!content.parts.is_empty());
    }

    #[tokio::test]
    async fn test_chat_max_steps_limit_reached() {
        // Create responses that only have reasoning (no final text)
        let reasoning_content = Content {
            parts: Parts(vec![PartEnum::from_reasoning("Still thinking...")]),
            role: RoleEnum::Model,
            complete_reason: CompleteReasonEnum::None,
            ..Default::default()
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

    #[test]
    fn test_extract_structured_candidate_with_structured_part() {
        use serde_json::json;
        let content = Content {
            parts: Parts(vec![PartEnum::from_structured(
                json!({"key": "value", "number": 42}),
            )]),
            ..Default::default()
        };

        let result = extract_structured_candidate(&content);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), json!({"key": "value", "number": 42}));
    }

    #[test]
    fn test_extract_structured_candidate_with_text_json() {
        use serde_json::json;
        let json_text = r#"{"name": "test", "count": 5}"#;
        let content = Content {
            parts: Parts(vec![PartEnum::from_text(json_text)]),
            ..Default::default()
        };

        let result = extract_structured_candidate(&content);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), json!({"name": "test", "count": 5}));
    }

    #[test]
    fn test_extract_structured_candidate_with_invalid_json_text() {
        let content = Content {
            parts: Parts(vec![PartEnum::from_text("This is not JSON")]),
            ..Default::default()
        };

        let result = extract_structured_candidate(&content);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_structured_candidate_empty_content() {
        let content = Content {
            parts: Parts(vec![]),
            ..Default::default()
        };

        let result = extract_structured_candidate(&content);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_structured_candidate_with_other_part_types() {
        let content = Content {
            parts: Parts(vec![PartEnum::from_reasoning("reasoning text")]),
            ..Default::default()
        };

        let result = extract_structured_candidate(&content);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_structured_candidate_complex_nested_json() {
        use serde_json::json;
        let complex = json!({
            "nested": {
                "array": [1, 2, {"inner": "value"}],
                "boolean": true
            },
            "null_field": null
        });

        let content = Content {
            parts: Parts(vec![PartEnum::from_structured(complex.clone())]),
            ..Default::default()
        };

        let result = extract_structured_candidate(&content);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), complex);
    }

    #[test]
    fn test_extract_structured_candidate_multiple_parts_uses_last() {
        use serde_json::json;
        let content = Content {
            parts: Parts(vec![
                PartEnum::from_text("first part"),
                PartEnum::from_reasoning("middle part"),
                PartEnum::from_structured(json!({"last": "part"})),
            ]),
            ..Default::default()
        };

        let result = extract_structured_candidate(&content);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), json!({"last": "part"}));
    }
}
