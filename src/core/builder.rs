use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use tools_rs::ToolCollection;

use crate::{
    chat::{
        complete::Chat,
        state::{Embedded, Streamed, Structured, Unstructured},
    },
    traits::{ChatProvider, ChatStreamProvider},
    types::{
        callback::{CallbackStrategy, RetryStrategy},
        options::ChatOptions,
    },
};

pub struct ChatBuilder<CP: ChatProvider, Output = Unstructured> {
    model: Option<CP>,
    output_shape: Option<schemars::Schema>,
    model_options: Option<ChatOptions>,
    max_steps: Option<u16>,
    max_retries: Option<u16>,
    retry_strategy: Option<RetryStrategy>,
    before_strategy: Option<CallbackStrategy>,
    after_strategy: Option<CallbackStrategy>,
    tools: Option<ToolCollection>,
    _output: std::marker::PhantomData<Output>,
}

impl<CP: ChatProvider> ChatBuilder<CP, Unstructured> {
    pub fn new() -> Self {
        ChatBuilder {
            _output: std::marker::PhantomData,
            ..Default::default()
        }
    }

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
            before_strategy: self.before_strategy,
            after_strategy: self.after_strategy,
            output_shape: Some(shape),
            tools: self.tools,
            model_options: self.model_options,
            _output: std::marker::PhantomData,
        }
    }

    pub fn with_streamed_response(self) -> ChatBuilder<CP, Streamed>
    where
        CP: ChatStreamProvider,
    {
        if self.output_shape.is_some() {
            println!(
                "Warning: Cannot call streamed responses with structured outputs. Output shape will be set to None"
            );
        }

        ChatBuilder {
            model: self.model,
            max_steps: self.max_steps,
            max_retries: self.max_retries,
            retry_strategy: self.retry_strategy,
            before_strategy: self.before_strategy,
            after_strategy: self.after_strategy,
            output_shape: None, // No shape for pure streaming
            tools: self.tools,
            model_options: self.model_options,
            _output: std::marker::PhantomData,
        }
    }

    pub fn with_embeddings(self) -> ChatBuilder<CP, Embedded> {
        if self.output_shape.is_some() {
            println!(
                "Warning: Cannot call embedding responses with structured outputs. Output shape will be set to None"
            );
        }

        ChatBuilder {
            model: self.model,
            max_retries: self.max_retries,
            retry_strategy: self.retry_strategy,
            before_strategy: self.retry_strategy,
            after_strategy: self.after_strategy,
            ..Default::default()
        }
    }
}

impl<CP: ChatProvider, Output> ChatBuilder<CP, Output> {
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

    pub fn with_model(mut self, model: CP) -> Self {
        self.model = Some(model);
        self
    }

    pub fn with_options(mut self, options: ChatOptions) -> Self {
        self.model_options = Some(options);
        self
    }

    pub fn build(self) -> Chat<CP, Output> {
        Chat {
            model: self.model.expect("Need to set a model"),
            output_shape: self.output_shape,
            max_steps: self.max_steps,
            max_retries: self.max_retries,
            retry_strategy: self.retry_strategy,
            before_strategy: self.before_strategy,
            after_strategy: self.after_strategy,
            tools: self.tools,
            model_options: self.model_options,
            _output: std::marker::PhantomData,
        }
    }
}

impl<CP: ChatProvider> Default for ChatBuilder<CP, Unstructured> {
    fn default() -> Self {
        ChatBuilder {
            model: None,
            output_shape: None,
            model_options: None,
            max_steps: None,
            max_retries: None,
            retry_strategy: None,
            before_strategy: None,
            after_strategy: None,
            tools: None,
            _output: std::marker::PhantomData,
        }
    }
}
