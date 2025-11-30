use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use tools_rs::ToolCollection;

use crate::core::{
    lib::{ChatError, ChatOptions, ChatProvider},
    messages::{
        Messages,
        content::Content,
        parts::{PartEnum, Parts},
    },
};

pub struct Unstructured;
pub struct Structured<T>(std::marker::PhantomData<T>);

#[derive(Default)]
pub struct Chat<CP: ChatProvider, Output = Unstructured> {
    model: CP,
    output_shape: Option<schemars::Schema>,
    model_options: Option<ChatOptions>,
    max_steps: Option<i16>,
    max_retries: Option<i16>,
    tools: Option<ToolCollection>,
    _output: std::marker::PhantomData<Output>,
}

impl<CP: ChatProvider> Chat<CP, Unstructured> {
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
}

impl<CP: ChatProvider, T> Chat<CP, Structured<T>>
where
    T: DeserializeOwned + JsonSchema,
{
        pub async fn complete(&mut self, messages: &mut Messages) -> Result<T, ChatError> {
        let max_retries = self.max_retries.unwrap_or(1);
        let mut last_err: Option<ChatError> = None;

        for _ in 0..max_retries {
            let retry_messages = messages.clone();

            match self.call_loop(&retry_messages).await {
                Ok(content) => {
                    // Extract and parse structured output
                    match extract_structured_candidate(&content) {
                        Some(value) => match serde_json::from_value::<T>(value.clone()) {
                            Ok(structured) => return Ok(structured),
                            Err(e) => {
                                last_err = Some(ChatError::InvalidResponse(format!(
                                    "Failed to parse structured output: {}. JSON: {}",
                                    e, value
                                )));
                                continue;
                            }
                        },
                        None => {
                            last_err = Some(ChatError::InvalidResponse(
                                "Response did not contain valid structured output".to_string(),
                            ));
                            continue;
                        }
                    }
                }
                Err(err) => {
                    last_err = Some(err);
                    continue;
                }
            }
        }

        Err(last_err.unwrap_or(ChatError::RateLimited))
    }
}

// Shared implementation for both output types
impl<CP: ChatProvider, Output> Chat<CP, Output> {
    /// Calls each function-call part in `content` using the chat's configured tool collection and
    /// returns the collected function responses as `Parts`.
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

    /// Runs the model completion loop until a terminal response is produced or the allowed steps are exhausted.
    async fn call_loop(&mut self, messages: &Messages) -> Result<Content, ChatError> {
        let mut inner_messages = messages.clone();
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
                    PartEnum::Structured(_structured) => {
                        return Ok(response);
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
        }
        Err(ChatError::RateLimited)
    }
}

pub struct ChatBuilder<CP: ChatProvider, Output = Unstructured> {
    model: Option<CP>,
    output_shape: Option<schemars::Schema>,
    model_options: Option<ChatOptions>,
    max_steps: Option<i16>,
    max_retries: Option<i16>,
    tools: Option<ToolCollection>,
    _output: std::marker::PhantomData<Output>,
}

impl<CP: ChatProvider> ChatBuilder<CP, Unstructured> {
    /// Create a new ChatBuilder with all configuration fields unset.
    pub fn new() -> Self {
        ChatBuilder {
            model: None,
            max_steps: None,
            max_retries: None,
            output_shape: None,
            tools: None,
            model_options: None,
            _output: std::marker::PhantomData,
        }
    }

    /// Configure the builder to expect structured JSON output shaped like `T`.
    ///
    /// This consumes the builder and returns a new `ChatBuilder<CP, Structured<T>>` that will
    /// return `T` directly from `complete()` calls instead of `Content`.
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
    /// let chat = ChatBuilder::new()
    ///     .with_structured_output::<MyOutput>()
    ///     // .with_model(...)
    ///     .build();
    ///
    /// // Now chat.complete() returns Result<MyOutput, ChatError>
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
            output_shape: Some(shape),
            tools: self.tools,
            model_options: self.model_options,
            _output: std::marker::PhantomData,
        }
    }
}

impl<CP: ChatProvider, Output> ChatBuilder<CP, Output> {
    /// Sets the maximum number of iterations the chat loop will perform when running `call_loop`.
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

    pub fn with_options(mut self, options: ChatOptions) -> Self {
        self.model_options = Some(options);
        self
    }

    /// Creates a Chat from this builder, consuming the builder.
    ///
    /// # Panics
    ///
    /// Panics if a model was not provided via `with_model`.
    pub fn build(self) -> Chat<CP, Output> {
        Chat {
            model: self.model.expect("Need to set a model"),
            output_shape: self.output_shape,
            max_steps: self.max_steps,
            max_retries: self.max_retries,
            tools: self.tools,
            model_options: self.model_options,
            _output: std::marker::PhantomData,
        }
    }
}

impl<CP: ChatProvider> Default for ChatBuilder<CP, Unstructured> {
    fn default() -> Self {
        ChatBuilder::new()
    }
}

/// Extracts a JSON candidate from the last part of `content`.
///
/// If the last part is `PartEnum::Structured`, returns its contained `serde_json::Value`.
/// If the last part is `PartEnum::Text` and the text parses as JSON, returns the parsed `Value`.
/// Returns `None` if there is no last part or the last part is neither `Structured` nor parsable `Text`.
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

        fn single_response(text: &str) -> Self {
            let content = Content {
                parts: Parts(vec![PartEnum::from_text(text)]),
                role: RoleEnum::Model,
                complete_reason: CompleteReasonEnum::Stop,
            };
            MockChatProvider::new(vec![content])
        }

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
                Ok(self.responses.last().unwrap().clone())
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

        let content = result.unwrap();
        assert_eq!(content.role, RoleEnum::Model);
    }

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
        assert_eq!(output.answer, "42");
        assert_eq!(output.confidence, 0.95);
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

        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.answer, "The answer");
        assert_eq!(output.confidence, 0.8);
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

        match result.unwrap_err() {
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

        match result.unwrap_err() {
            ChatError::InvalidResponse(msg) => {
                assert!(msg.contains("Failed to parse structured output"));
            }
            _ => panic!("Expected InvalidResponse error"),
        }
    }

    #[tokio::test]
    async fn test_structured_complete_with_retries() {
        let invalid = Content {
            parts: Parts(vec![PartEnum::from_text("invalid")]),
            role: RoleEnum::Model,
            complete_reason: CompleteReasonEnum::Stop,
        };

        let valid = Content {
            parts: Parts(vec![PartEnum::from_structured(serde_json::json!({
                "answer": "success",
                "confidence": 1.0
            }))]),
            role: RoleEnum::Model,
            complete_reason: CompleteReasonEnum::Stop,
        };

        let model = MockChatProvider::new(vec![invalid, valid]);

        let mut chat = ChatBuilder::new()
            .with_structured_output::<TestOutput>()
            .with_max_retries(3)
            .with_model(model)
            .build();

        let mut messages = Messages::default();
        messages.push(crate::messages::content::from_user(vec!["Question"]));

        let result = chat.complete(&mut messages).await;
        assert!(result.is_ok());

        let output = result.unwrap();
        assert_eq!(output.answer, "success");
    }
}

