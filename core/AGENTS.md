# Core Crate (`chat-core`)

The foundational crate defining all traits, types, and the `Chat` engine. Providers depend on this crate — it never depends on any provider.

## Architecture

```
src/
├── lib.rs              # Public re-exports
├── traits.rs           # CompletionProvider, StreamProvider, EmbeddingsProvider
├── builder.rs          # ChatBuilder (type-state pattern)
├── error.rs            # ChatError, ChatFailure
├── macros/mod.rs       # Procedural macros (retry_strategy!)
├── utils.rs            # Internal helpers
├── chat/
│   ├── mod.rs          # Chat<CP, Output> struct
│   ├── completion.rs   # Unstructured + Structured completion logic
│   ├── embed.rs        # Embedding logic
│   ├── state.rs        # Type states: Unstructured, Structured<T>, Streamed, Embedded
│   └── stream.rs       # Streaming logic (feature-gated on "stream")
└── types/
    ├── messages/       # Messages, Content, PartEnum, File, Embeddings, Reasoning
    ├── response.rs     # ChatResponse, StructuredResponse<T>, EmbeddingsResponse, StreamEvent
    ├── options.rs      # ChatOptions (temperature, max_tokens, top_p, metadata)
    ├── callback.rs     # CallbackStrategy, RetryStrategy
    └── metadata/       # Metadata, Usage (token counts)
```

## Key Traits

All provider crates must implement these from `traits.rs`:

### `CompletionProvider` (required)

```rust
#[async_trait]
pub trait CompletionProvider: Send + Sync {
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tools: Option<&ToolCollection>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure>;
}
```

### `StreamProvider` (optional, feature-gated on `stream`)

```rust
#[async_trait]
pub trait StreamProvider: Send + Sync {
    async fn stream(
        &mut self,
        messages: &mut Messages,
        tools: Option<&ToolCollection>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError>;

    fn on_stream_done(&mut self, _response: &ChatResponse) {}
}
```

### `EmbeddingsProvider` (optional)

```rust
#[async_trait]
pub trait EmbeddingsProvider: Send + Sync {
    async fn embed(&self, messages: &mut Messages) -> Result<EmbeddingsResponse, ChatFailure>;
}
```

## Type-State Builder

`ChatBuilder<CP, Output>` uses phantom types to enforce valid configurations at compile time:

- `Unstructured` (default) — `.complete()` returns `ChatResponse`
- `Structured<T>` — `.complete()` returns `StructuredResponse<T>` (T: JsonSchema + DeserializeOwned)
- `Streamed` — `.stream()` returns `BoxStream<StreamEvent>` (requires CP: StreamProvider)
- `Embedded` — `.embed()` returns `EmbeddingsResponse`

State transitions are one-way from `Unstructured`: calling `.with_structured_output::<T>()`, `.with_streamed_response()`, or `.with_embeddings()` consumes the builder into the new state.

## Message System

`Messages` wraps `Vec<Content>`. Pushing a `Content` with the same role as the last message merges them (appends parts) rather than creating a new entry.

`Content` has a `role` (User, System, Model) and `parts` containing `PartEnum` variants:
- `Text`, `Reasoning`, `FunctionCall`, `FunctionResponse`, `Structured(Value)`, `File`, `Embeddings`

`File` is either `Url { url, mimetype }` or `Bytes { bytes, mimetype }`.

## Tool Calling Loop

`Chat::call_loop()` in `completion.rs` drives multi-step tool use:
1. Calls `provider.complete()`
2. If response contains `FunctionCall` parts, executes them via `ToolCollection`
3. Appends tool results as `FunctionResponse` parts
4. Loops back to step 1 (up to `max_steps`)

## Error Model

- `ChatError` — enum of error variants (Network, Provider, RateLimited, MaxStepsExceeded, InvalidResponse, Callback, Other)
- `ChatFailure` — wraps `ChatError` with optional `Metadata`, so partial token usage is preserved even on failure

## Caveats

- Streaming and structured output are mutually exclusive. Calling `.with_streamed_response()` after `.with_structured_output()` is prevented by the type system, but going through `BaseConfig` then streaming will clear any output shape with a printed warning.
- `Messages::push()` silently merges consecutive same-role messages. This is intentional for tool call flows but can be surprising if you expect separate entries.
- The `metadata` field on responses uses `HashMap<String, Value>` for provider-specific data (`specific` field) — there is no schema enforcement on these.
- Feature flag `stream` must be enabled at every layer: `chat-core/stream`, and propagated through `chat-rs/stream` to each provider.
