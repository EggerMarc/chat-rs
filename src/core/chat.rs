use std::sync::Arc;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
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
    lib::{ChatFailure, ChatResponse, EmbeddingsResponse},
    metadata::Metadata,
    retry::{RetryContext, RetryStrategy},
};

pub struct Unstructured;
pub struct Structured<T>(std::marker::PhantomData<T>);

#[derive(Default)]
pub struct Chat<CP: ChatProvider, Output = Unstructured> {
    model: CP,
    output_shape: Option<schemars::Schema>,
    model_options: Option<ChatOptions>,
    max_steps: Option<u16>,
    max_retries: Option<u16>,
    retry_strategy: Option<RetryStrategy>,
    tools: Option<ToolCollection>,
    _output: std::marker::PhantomData<Output>,
}

impl<CP: ChatProvider> Chat<CP, Unstructured> {
    /// Perform a model-driven completion loop and return the final ChatResponse.
    ///
    /// Attempts completion up to the configured `max_retries` (default 1). On the first successful
    /// completion returns the produced content and aggregated metadata. If all attempts fail, returns
    /// a `ChatFailure` containing the last observed `ChatError` (or `ChatError::RateLimited` if none).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use crate::core::chat::{Chat, Messages};
    /// # async fn example(mut chat: Chat<impl crate::core::lib::ChatProvider, _>, mut messages: Messages) {
    /// let result = chat.complete(&mut messages).await;
    /// match result {
    ///     Ok(response) => println!("Got content: {:?}", response.content),
    ///     Err(failure) => eprintln!("Completion failed: {:?}", failure.err),
    /// }
    /// # }
    /// ```
    pub async fn complete(&mut self, messages: &mut Messages) -> Result<ChatResponse, ChatFailure> {
        let max_retries = self.max_retries.unwrap_or(1);

        let mut last_err: Option<ChatError> = None;
        let mut last_metadata: Option<Metadata> = None;

        for idx in 0..max_retries {
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
                    if let Some(metadata) = err.clone().metadata {
                        match &mut last_metadata {
                            Some(existing) => {
                                existing.extend(&metadata);
                            }
                            None => {
                                last_metadata = Some(metadata);
                            }
                        }
                    }

                    last_err = Some(err.err.clone());

                    if max_retries > 1 {
                        let ctx = RetryContext {
                            idx,
                            failure: err.clone(),
                            messages: Arc::new(messages.to_owned()),
                        };

                        if let Some(strategy) = self.retry_strategy.as_mut() {
                            strategy(ctx).await;
                        }
                    }
                }
            }
        }

        Err(ChatFailure {
            metadata: last_metadata,
            err: last_err.unwrap_or(ChatError::RateLimited),
        })
    }

    /// Extracts embeddings from the model's response to the provided messages.
    ///
    /// Calls the provider with the given messages and returns the embeddings part of the final response along with any associated metadata.
    ///
    /// # Returns
    ///
    /// `EmbeddingsResponse` containing the embeddings and any response metadata on success.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn example(chat: &crate::Chat<impl crate::ChatProvider>) -> Result<(), crate::ChatFailure> {
    /// let mut messages = crate::Messages::default();
    /// let embeddings_resp = chat.embed(&mut messages).await?;
    /// assert!(!embeddings_resp.embeddings.is_empty());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn embed(&self, messages: &mut Messages) -> Result<EmbeddingsResponse, ChatFailure> {
        let response = self.model.complete(messages, None, None, None).await?;

        let metadata = response.metadata;
        let embeddings_part = response.content.parts.last().ok_or_else(|| ChatFailure {
            err: ChatError::InvalidResponse("No parts in response".to_string()),
            metadata: metadata.clone(),
        })?;

        match embeddings_part {
            PartEnum::Embeddings(embeddings) => Ok(EmbeddingsResponse {
                metadata,
                embeddings: embeddings.clone(),
            }),
            _ => Err(ChatFailure {
                err: ChatError::InvalidResponse("Response was not embeddings".to_string()),
                metadata,
            }),
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
    /// Completes a chat interaction and returns the final response deserialized as `T`.
    ///
    /// Attempts up to `max_retries` times (default 1) to drive the model to a terminal response,
    /// extract a structured JSON candidate from the final content, and deserialize it into `T`.
    /// If a structured candidate is missing or cannot be parsed, this returns `ChatError::InvalidResponse`.
    /// If the model produces an error on all retries, the last model error is returned; if no error is recorded
    /// this returns `ChatError::RateLimited`.
    ///
    /// # Examples
    ///
    /// ```
    /// // Synchronous example using a simple executor to run the async call.
    /// // `chat` is a `Chat<_, Structured<MyType>>` and `msgs` is a `Messages` value prepared earlier.
    /// let result = futures::executor::block_on(async {
    ///     chat.complete(&mut msgs).await
    /// });
    /// match result {
    ///     Ok(value) => println!("Parsed structured value: {:?}", value),
    ///     Err(e) => eprintln!("Chat failed: {:?}", e),
    /// }
    /// ```
    pub async fn complete(
        &mut self,
        messages: Messages,
    ) -> Result<StructuredResponse<T>, ChatFailure> {
        let max_retries = self.max_retries.unwrap_or(1);
        let mut last_err: Option<ChatFailure> = None;
        let mut last_metadata: Option<Metadata> = None;

        for idx in 0..max_retries {
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

                    if let Some(value) = extract_structured_candidate(&response.content) {
                        match serde_json::from_value::<T>(value.clone()) {
                            Ok(structured) => {
                                return Ok(StructuredResponse {
                                    content: structured,
                                    metadata: last_metadata,
                                });
                            }
                            Err(err) => {
                                last_err = Some(ChatFailure {
                                    err: ChatError::InvalidResponse(format!(
                                        "Failed to parse structured output: {} on result: {}",
                                        err, value
                                    )),
                                    metadata: last_metadata.clone(),
                                })
                            }
                        }
                    } else {
                        last_err = Some(ChatFailure {
                            err: ChatError::InvalidResponse(
                                "Response did not contain valid structured output".to_string(),
                            ),
                            metadata: last_metadata.clone(),
                        });
                        continue;
                    }
                }
                Err(err) => {
                    last_err = Some(err.clone());
                    if max_retries > 1 {
                        let ctx = RetryContext {
                            idx,
                            failure: err.clone(),
                            messages: Arc::new(messages.to_owned()),
                        };

                        if let Some(strategy) = self.retry_strategy.as_mut() {
                            strategy(ctx).await;
                        }
                    }

                    continue;
                }
            }
        }

        Err(last_err.unwrap_or(ChatFailure {
            metadata: last_metadata,
            err: ChatError::RateLimited,
        }))
    }
}

// Shared implementation for both output types
impl<CP: ChatProvider, Output> Chat<CP, Output> {
    /// Calls any function-call parts found in `content` using the chat's configured tool collection and returns the collected function responses as `Parts`.
    ///
    /// If `content` contains function-call parts and no tool collection is configured, this returns `Err(ChatError::InvalidResponse)` with a message indicating the missing tool collection. Tool call failures are mapped to `ChatError::InvalidResponse("Tools error")`.
    ///
    /// # Examples
    ///
    /// ```
    /// # // This example assumes types `Chat`, `Content`, and `Parts` are in scope and that
    /// # // `chat` is a properly constructed `Chat` instance. It demonstrates that when
    /// # // `content` contains no function-call parts, an empty `Parts` is returned.
    /// # let chat = /* construct chat with or without tools */ unimplemented!();
    /// # let content = Content::default();
    /// let parts = futures::executor::block_on(chat.tool_call(&content)).unwrap();
    /// assert_eq!(parts, Parts::default());
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
                    .map_err(|_err| ChatError::InvalidResponse("Tools error".to_string()))?,
            ));
        }
        Ok(frs)
    }

    /// Drives the model until a terminal content part is produced or the step budget is exhausted.
    ///
    /// Calls the provider's completion API repeatedly (up to the configured `max_steps`) while
    /// appending each model response to the message history. After each model call it:
    /// - invokes configured tools and appends any tool-produced parts to the response,
    /// - treats a last-part of type `Text` or `Structured` as terminal and returns that `Content`,
    /// - treats a last-part of type `Reasoning` by pushing the reasoning back into the response parts
    ///   and continuing the loop.
    ///If a model response contains no parts, returns `ChatError::InvalidResponse`. If no terminal
    ///content is produced within the allowed steps, returns `ChatError::RateLimited`.
    /// # Examples
    ///
    /// ```
    /// # use std::sync::Arc;
    /// # use crate::core::{Chat, ChatBuilder, Messages, Unstructured};
    /// # async fn example(mut chat: Chat<impl crate::core::ChatProvider, Unstructured>, mut msgs: Messages) -> Result<(), crate::core::ChatError> {
    /// let content = chat.call_loop(&msgs).await?;
    /// // content now contains a terminal Text or Structured last part
    /// # Ok(())
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

            let response_metadata = response.metadata.clone();

            if let Some(metadata) = response_metadata {
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
                    PartEnum::Text(_text) => {
                        return Ok(ChatResponse {
                            metadata: last_metadata,
                            content: response.content,
                        });
                    }
                    PartEnum::Reasoning(reasoning) => {
                        response
                            .content
                            .parts
                            .push(PartEnum::from_reasoning(reasoning.to_owned()));
                    }
                    PartEnum::Structured(_structured) => {
                        return Ok(ChatResponse {
                            metadata: last_metadata,
                            content: response.content,
                        });
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

pub struct ChatBuilder<CP: ChatProvider, Output = Unstructured> {
    model: Option<CP>,
    output_shape: Option<schemars::Schema>,
    model_options: Option<ChatOptions>,
    max_steps: Option<u16>,
    max_retries: Option<u16>,
    retry_strategy: Option<RetryStrategy>,
    tools: Option<ToolCollection>,
    _output: std::marker::PhantomData<Output>,
}

impl<CP: ChatProvider> ChatBuilder<CP, Unstructured> {
    /// Create a ChatBuilder configured for unstructured output with all configuration fields unset.
    ///
    /// This returns a builder where `model`, `max_steps`, `max_retries`, `output_shape`,
    /// `tools`, and `model_options` are all `None`, ready to be configured.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let builder = ChatBuilder::<MockProvider>::new();
    /// let configured = builder.with_max_steps(3).with_model(MockProvider::default());
    /// ```
    pub fn new() -> Self {
        ChatBuilder {
            model: None,
            max_steps: None,
            max_retries: None,
            retry_strategy: None,
            output_shape: None,
            tools: None,
            model_options: None,
            _output: std::marker::PhantomData,
        }
    }

    /// Configure the builder to produce structured JSON output shaped like `T`.
    ///
    /// Consumes the builder and returns a new `ChatBuilder<CP, Structured<T>>` whose
    /// `complete()` will deserialize model responses into `T`.
    ///
    /// # Examples
    ///
    /// ```
    /// use schemars::JsonSchema;
    /// use serde::Deserialize;
    ///
    /// #[derive(JsonSchema, Deserialize)]
    /// struct MyOutput {
    ///     pub answer: String,
    ///     pub confidence: f64,
    /// }
    ///
    /// let builder = ChatBuilder::new().with_structured_output::<MyOutput>();
    /// // builder.with_model(...).build() -> Chat<_, Structured<MyOutput>>
    /// ```
    pub fn with_structured_output<T>(self) -> ChatBuilder<CP, Structured<T>>
    where
        T: JsonSchema + DeserializeOwned,
    {
        let shape = schemars::schema_for!(T);

        ChatBuilder {
            model: self.model,
            max_steps: self.max_steps,
            max_retries: self.max_retries,
            retry_strategy: self.retry_strategy,
            output_shape: Some(shape),
            tools: self.tools,
            model_options: self.model_options,
            _output: std::marker::PhantomData,
        }
    }
}

impl<CP: ChatProvider, Output> ChatBuilder<CP, Output> {
    /// Sets the maximum number of iterations the chat loop will perform when running `call_loop`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let builder = ChatBuilder::<MockProvider>::new().with_max_steps(3);
    /// ```
    pub fn with_max_steps(mut self, max_steps: u16) -> Self {
        self.max_steps = Some(max_steps);
        self
    }

    pub fn with_max_retries(mut self, max_retries: u16) -> Self {
        self.max_retries = Some(max_retries);
        self
    }

    pub fn with_tools(mut self, tools: ToolCollection) -> Self {
        self.tools = Some(tools);
        self
    }

    pub fn with_retry_strategy(mut self, retry_strategy: RetryStrategy) -> Self {
        self.retry_strategy = Some(retry_strategy);
        self
    }

    /// Set the chat provider implementation on the builder.
    ///
    /// # Examples
    ///
    /// ```
    /// let builder = ChatBuilder::new().with_model(my_model);
    /// ```
    pub fn with_model(mut self, model: CP) -> Self {
        self.model = Some(model);
        self
    }

    /// Set model-level chat options on the builder.
    ///
    /// The provided `ChatOptions` will be used as the model options for the `Chat` constructed by this builder.
    ///
    /// # Examples
    ///
    /// ```
    /// let builder = ChatBuilder::new().with_options(ChatOptions { /* fields */ });
    /// let chat = builder.with_model(mock_provider).build();
    /// ```
    pub fn with_options(mut self, options: ChatOptions) -> Self {
        self.model_options = Some(options);
        self
    }

    /// Constructs a Chat instance from this builder, consuming the builder.
    ///
    /// # Panics
    ///
    /// Panics if a model was not provided via `with_model`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// let builder = ChatBuilder::new().with_model(my_model);
    /// let chat = builder.build();
    /// ```
    pub fn build(self) -> Chat<CP, Output> {
        Chat {
            model: self.model.expect("Need to set a model"),
            output_shape: self.output_shape,
            max_steps: self.max_steps,
            max_retries: self.max_retries,
            retry_strategy: self.retry_strategy,
            tools: self.tools,
            model_options: self.model_options,
            _output: std::marker::PhantomData,
        }
    }
}

impl<CP: ChatProvider> Default for ChatBuilder<CP, Unstructured> {
    /// Creates a default ChatBuilder for unstructured output.
    ///
    /// # Examples
    ///
    /// ```
    /// let b1 = ChatBuilder::<()>::new();
    /// let b2 = ChatBuilder::<()>::default();
    /// // Both constructors produce a builder configured for unstructured output.
    /// ```
    fn default() -> Self {
        ChatBuilder::new()
    }
}

/// Extracts a JSON value candidate from the last part of a `Content`.
///
/// If the last part is `PartEnum::Structured`, returns its contained `serde_json::Value`.
/// If the last part is `PartEnum::Text` and that text parses as JSON, returns the parsed `Value`.
/// Returns `None` when there is no last part or the last part is neither structured nor parsable JSON.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// let c = Content { parts: vec![PartEnum::Text("{\"a\":1}".into())], Content::default()};
/// assert_eq!(extract_structured_candidate(&c), Some(json!({"a":1})));
///
/// // Text containing JSON
/// let content = Content { parts: vec![PartEnum::Text("{\"ok\":true}".into())] };
/// let v = extract_structured_candidate(&content).unwrap();
/// assert_eq!(v, json!({"ok": true}));
///
/// // Already structured
/// let content = Content { parts: vec![PartEnum::Structured(json!({"n": 2}))] };
/// let v = extract_structured_candidate(&content).unwrap();
/// assert_eq!(v, json!({"n": 2}));
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
    use serde::Deserialize;

    // Test struct for structured output
    #[derive(JsonSchema, Deserialize, Debug, PartialEq)]
    struct TestOutput {
        answer: String,
        confidence: f64,
    }

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

        /// Creates a MockChatProvider that will return a single model text response.
        ///
        /// The returned provider will yield one Content with the given text as a Text part,
        /// role set to Model, and completion reason set to Stop.
        ///
        /// # Examples
        ///
        /// ```
        /// let provider = MockChatProvider::single_response("hello");
        /// let mut messages = Messages::new();
        /// let content = futures::executor::block_on(async { provider.complete(&messages, None, None, None).await }).unwrap();
        /// assert!(content.parts.last().unwrap().is_text());
        /// ```
        fn single_response(text: &str) -> Self {
            let content = Content {
                parts: Parts(vec![PartEnum::from_text(text)]),
                role: RoleEnum::Model,
                complete_reason: CompleteReasonEnum::Stop,
            };
            MockChatProvider::new(vec![content])
        }

        /// Creates a MockChatProvider that returns a single model response containing the provided structured JSON value.
        ///
        /// The generated provider yields one Content whose last part is a Structured value, with role set to `Model` and completion reason `Stop`.
        ///
        /// # Examples
        ///
        /// ```
        /// let value = serde_json::json!({"answer": "42", "confidence": 0.99});
        /// let provider = MockChatProvider::structured_response(value.clone());
        /// // `provider` will return a single Content whose structured part equals `value`.
        /// ```
        fn structured_response(value: serde_json::Value) -> Self {
            let content = Content {
                parts: Parts(vec![PartEnum::from_structured(value)]),
                role: RoleEnum::Model,
                complete_reason: CompleteReasonEnum::Stop,
            };
            MockChatProvider::new(vec![content])
        }
    }

    #[async_trait]
    impl ChatProvider for MockChatProvider {
        /// Mock `complete` implementation that returns predefined responses in sequence.
        ///
        /// The method returns the next `Content` from the provider's internal `responses` list on each call;
        /// once the list is exhausted it repeatedly returns the last entry. Each invocation increments an
        /// internal call counter to advance through the sequence.
        ///
        /// # Examples
        ///
        /// ```
        /// use futures::executor::block_on;
        /// // Assume `mock` is an instance of the mock provider with at least one response:
        /// // let mock = MockChatProvider::single_response("hello".to_string());
        /// // let content = block_on(mock.complete(&messages, None, None, None)).unwrap();
        /// // assert_eq!(content.parts.first().unwrap().as_text().unwrap(), "hello");
        /// ```
        async fn complete(
            &self,
            _messages: &Messages,
            _tools: Option<&ToolCollection>,
            _options: Option<&ChatOptions>,
            _schema: Option<&schemars::Schema>,
        ) -> Result<ChatResponse, ChatFailure> {
            let mut count = self.call_count.lock().unwrap();
            let idx = *count;
            *count += 1;

            if idx < self.responses.len() {
                Ok(ChatResponse {
                    content: self.responses[idx].clone(),
                    metadata: Some(Metadata::default()),
                })
            } else {
                // Return last response if called more times than we have responses
                Ok(ChatResponse {
                    content: self.responses.last().unwrap().clone(),
                    metadata: Some(Metadata::default()),
                })
            }
        }
    }

    #[tokio::test]
    async fn test_unstructured_complete() {
        let model = MockChatProvider::single_response("Hello, world!");
        let mut chat = ChatBuilder::new().with_model(model).build();

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Hi"]));

        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());

        let result = result.unwrap();
        assert_eq!(result.content.role, RoleEnum::Model);
        assert_eq!(
            result.content.parts.text_response().unwrap().as_str(),
            "Hello, world!"
        );
    }

    /// Verifies that a chat configured for structured output correctly deserializes a structured part into the target type.
    ///
    /// # Examples
    ///
    /// ```
    /// // Build a mock provider that returns a structured JSON part matching `TestOutput`.
    /// let test_value = serde_json::json!({
    ///     "answer": "42",
    ///     "confidence": 0.95
    /// });
    ///
    /// let model = MockChatProvider::structured_response(test_value);
    /// let mut chat = ChatBuilder::new()
    ///     .with_structured_output::<TestOutput>()
    ///     .with_model(model)
    ///     .build();
    ///
    /// let mut messages = Messages::default();
    /// messages.push(crate::messages::content::from_user(vec!["What is the answer?"]));
    ///
    /// let result = chat.complete(&mut messages).await;
    /// assert!(result.is_ok());
    /// let output = result.unwrap();
    /// assert_eq!(output.answer, "42");
    /// assert_eq!(output.confidence, 0.95);
    /// ```
    #[tokio::test]
    async fn test_structured_complete_with_structured_part() {
        let test_value = serde_json::json!({
            "answer": "42",
            "confidence": 0.95
        });

        let model = MockChatProvider::structured_response(test_value);
        let mut chat = ChatBuilder::new()
            .with_structured_output::<TestOutput>()
            .with_model(model)
            .build();

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec![
            "What is the answer?",
        ]));

        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.content.answer, "42");
        assert_eq!(output.content.confidence, 0.95);
    }

    #[tokio::test]
    async fn test_structured_complete_with_text_json() {
        let json_text = r#"{"answer": "The answer", "confidence": 0.8}"#;
        let model = MockChatProvider::single_response(json_text);

        let mut chat = ChatBuilder::new()
            .with_structured_output::<TestOutput>()
            .with_model(model)
            .build();

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Question"]));

        let result = chat.complete(messages).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.content.answer, "The answer");
        assert_eq!(output.content.confidence, 0.8);
    }

    #[tokio::test]
    async fn test_structured_complete_invalid_json() {
        let model = MockChatProvider::single_response("Not valid JSON");

        let mut chat = ChatBuilder::new()
            .with_structured_output::<TestOutput>()
            .with_model(model)
            .build();

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Question"]));

        let result = chat.complete(&mut messages).await;
        assert!(result.is_err());
        match result.unwrap_err().err {
            ChatError::InvalidResponse(msg) => {
                assert!(msg.contains("did not contain valid structured output"));
            }
            _ => panic!("Expected InvalidResponse error"),
        }
    }

    #[tokio::test]
    async fn test_structured_complete_wrong_schema() {
        let wrong_value = serde_json::json!({
            "wrong_field": "value"
        });

        let model = MockChatProvider::structured_response(wrong_value);

        let mut chat = ChatBuilder::new()
            .with_structured_output::<TestOutput>()
            .with_model(model)
            .build();

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Question"]));

        let result = chat.complete(&mut messages).await;
        assert!(result.is_err());
        match result.unwrap_err().err {
            ChatError::Other(msg) => {
                assert!(msg.contains("Structured output not yet implemented"));
            }
            _ => panic!("Expected InvalidResponse error"),
        }
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
        match result.unwrap_err().err {
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
