use std::collections::HashMap;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use tools_rs::ToolCollection;

#[cfg(feature = "stream")]
use crate::chat::state::{InputStreamed, Streamed};
use crate::{
    chat::{
        Chat,
        state::{Embedded, Structured, Unstructured},
    },
    traits::CompletionProvider,
    types::{
        callback::{CallbackStrategy, RetryStrategy},
        options::ChatOptions,
        tools::{ScopedCollection, TypedCollection},
    },
};

#[cfg(feature = "stream")]
use crate::traits::StreamProvider;

pub struct ChatBuilder<CP: CompletionProvider, Output = Unstructured> {
    model: Option<CP>,
    output_shape: Option<schemars::Schema>,
    model_options: Option<ChatOptions>,
    max_steps: Option<u16>,
    max_retries: Option<u16>,
    retry_strategy: Option<RetryStrategy>,
    before_strategy: Option<CallbackStrategy>,
    after_strategy: Option<CallbackStrategy>,
    scoped_collections: Vec<Box<dyn TypedCollection>>,
    _output: std::marker::PhantomData<Output>,
}

impl<CP: CompletionProvider> ChatBuilder<CP, Unstructured> {
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
            scoped_collections: self.scoped_collections,
            model_options: self.model_options,
            _output: std::marker::PhantomData,
        }
    }

    #[cfg(feature = "stream")]
    pub fn with_streamed_response(self) -> ChatBuilder<CP, Streamed>
    where
        CP: StreamProvider,
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
            output_shape: None,
            scoped_collections: self.scoped_collections,
            model_options: self.model_options,
            _output: std::marker::PhantomData,
        }
    }

    /// Transition into the input-stream type-state. The resulting
    /// `Chat<CP, InputStreamed>` exposes a `stream(&mut messages)` method
    /// that returns a [`ChatStream`](crate::chat::input::ChatStream): the
    /// output stream you iterate with `.next()`, carrying an input side you
    /// push to with `.send()` (or `split()` into independent handles).
    /// Audio rides as `PartEnum::File`, text as `PartEnum::Text`, tool
    /// results as `PartEnum::Tool`, etc. — no parallel input enum, and a
    /// continuous producer is mapped into `PartEnum` caller-side before it
    /// reaches `send`.
    #[cfg(feature = "stream")]
    pub fn with_input_stream(self) -> ChatBuilder<CP, InputStreamed>
    where
        CP: StreamProvider,
    {
        if self.output_shape.is_some() {
            println!(
                "Warning: Cannot call input-streamed responses with structured outputs. Output shape will be set to None"
            );
        }

        ChatBuilder {
            model: self.model,
            max_steps: self.max_steps,
            max_retries: self.max_retries,
            retry_strategy: self.retry_strategy,
            before_strategy: self.before_strategy,
            after_strategy: self.after_strategy,
            output_shape: None,
            scoped_collections: self.scoped_collections,
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
            before_strategy: self.before_strategy,
            after_strategy: self.after_strategy,
            output_shape: None,
            scoped_collections: Vec::new(),
            max_steps: None,
            model_options: self.model_options,
            _output: std::marker::PhantomData,
        }
    }
}

impl<CP: CompletionProvider, Output> ChatBuilder<CP, Output> {
    pub fn with_max_steps(mut self, max_steps: u16) -> Self {
        self.max_steps = Some(max_steps);
        self
    }

    pub fn with_max_retries(mut self, max_retries: u16) -> Self {
        self.max_retries = Some(max_retries);
        self
    }

    /// Convenience: wrap a plain `ToolCollection<NoMeta>` with an
    /// always-execute strategy. Equivalent to
    /// `.with_scoped_tools(ScopedCollection::auto_execute(tools))`.
    pub fn with_tools(mut self, tools: ToolCollection) -> Self {
        self.scoped_collections
            .push(Box::new(ScopedCollection::auto_execute(tools)));
        self
    }

    /// Attach a typed tool collection with a user-defined strategy.
    /// Multiple calls are additive — the resulting `Chat` routes each
    /// tool call to the collection that owns its name.
    pub fn with_scoped_tools<M, F>(mut self, scoped: ScopedCollection<M, F>) -> Self
    where
        M: Send + Sync + 'static,
        F: Fn(&tools_rs::FunctionCall, &M) -> crate::types::tools::Action + Send + Sync + 'static,
    {
        self.scoped_collections.push(Box::new(scoped));
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
        // Build routing table: tool name → index into scoped_collections.
        // Collisions across collections are a programming error — we
        // keep the first and ignore later ones with a warning. (Could
        // be promoted to a hard error via a builder method later.)
        let mut routing: HashMap<String, usize> = HashMap::new();
        for (idx, coll) in self.scoped_collections.iter().enumerate() {
            for name in coll.names() {
                if routing.contains_key(name) {
                    eprintln!(
                        "chat-rs: tool name `{name}` is registered in multiple scoped \
                         collections; keeping the first registration."
                    );
                    continue;
                }
                routing.insert(name.to_string(), idx);
            }
        }

        Chat {
            model: self.model.expect("Need to set a model"),
            output_shape: self.output_shape,
            max_steps: self.max_steps,
            max_retries: self.max_retries,
            retry_strategy: self.retry_strategy,
            before_strategy: self.before_strategy,
            after_strategy: self.after_strategy,
            scoped_collections: self.scoped_collections,
            routing,
            model_options: self.model_options,
            _output: std::marker::PhantomData,
        }
    }
}

impl<CP: CompletionProvider> Default for ChatBuilder<CP, Unstructured> {
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
            scoped_collections: Vec::new(),
            _output: std::marker::PhantomData,
        }
    }
}
