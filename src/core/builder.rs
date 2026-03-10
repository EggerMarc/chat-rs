use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use tools_rs::ToolCollection;

use crate::{
    callback::{CallbackStrategy, RetryStrategy},
    chat::{Chat, Structured, Unstructured},
    lib::{ChatOptions, ChatProvider},
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
            _output: std::marker::PhantomData,
            ..Default::default()
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
            before_strategy: self.before_strategy,
            after_strategy: self.after_strategy,
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
            before_strategy: self.before_strategy,
            after_strategy: self.after_strategy,
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
