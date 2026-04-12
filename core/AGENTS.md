# Core Crate (`chat-core`)

The foundational crate defining all traits, types, and the `Chat` engine. Providers depend on this crate — it never depends on any provider.

## Architecture

```
src/
├── lib.rs              # Public re-exports
├── traits.rs           # CompletionProvider, StreamProvider, EmbeddingsProvider, ChatProvider
├── builder.rs          # ChatBuilder (type-state pattern)
├── error.rs            # ChatError, ChatFailure, From<TransportError>
��── macros/mod.rs       # Procedural macros (retry_strategy!)
├── utils.rs            # Internal helpers
├── transport/
│   ├── mod.rs          # Public re-exports
│   ├── traits.rs       # Transport trait (send, stream)
│   ├── types.rs        # Request, Response, Event, EventStream, TransportError
│   └── sse.rs          # SseParser (shared utility for HTTP transports)
├── chat/
│   ├── mod.rs          # Chat<CP, Output> struct, scoped-tool dispatch, tool_call pre/post steps
│   ├── completion.rs   # Unstructured + Structured completion loop
│   ├── embed.rs        # Embedding logic
│   ├── state.rs        # Type states: Unstructured, Structured<T>, Streamed, Embedded
│   └── stream.rs       # Streaming loop with pause/resume (feature-gated on "stream")
└── types/
    ├── messages/       # Messages, Content, PartEnum, File, Embeddings, Reasoning, Tool
    ├── tools.rs        # ToolDeclarations, Action, ScopedCollection, strategy closures
    ├── response.rs     # ChatResponse, StructuredResponse<T>, EmbeddingsResponse,
    │                   # StreamEvent (incl. Paused), PauseReason
    ├── options.rs      # ChatOptions (temperature, max_tokens, top_p, metadata)
    ├── provider_meta.rs# ProviderMeta (provider self-description)
    ├── callback.rs     # CallbackStrategy, RetryStrategy
    └── metadata/       # Metadata, Usage (token counts)
```

## Key Traits

All provider crates implement these from `traits.rs`:

### `CompletionProvider` (required)

```rust
#[async_trait]
pub trait CompletionProvider: Send + Sync {
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure>;

    fn metadata(&self) -> Option<&ProviderMeta> { None }
}
```

Providers receive `tool_declarations` — a type-erased view that only exposes `.json()`. They never see strategies, metadata, or execution — the chat loop owns all of that.

### `StreamProvider` (optional, feature-gated on `stream`)

```rust
#[async_trait]
pub trait StreamProvider: Send + Sync {
    async fn stream(
        &mut self,
        messages: &mut Messages,
        tool_declarations: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
    ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError>;

    fn on_stream_done(&mut self, _response: &ChatResponse) {}
}
```

### `ChatProvider` (blanket supertrait, `stream` feature)

Any type implementing both `CompletionProvider` and `StreamProvider` automatically implements `ChatProvider`. The router uses this for its `StreamRouter`.

### `EmbeddingsProvider` (optional)

```rust
#[async_trait]
pub trait EmbeddingsProvider: Send + Sync {
    async fn embed(&mut self, messages: &mut Messages) -> Result<EmbeddingsResponse, ChatFailure>;
}
```

## Transport

The `Transport` trait (`transport/traits.rs`) abstracts over how requests are delivered and responses received. Providers are generic over `T: Transport`, so the same provider code works over HTTP, WebSocket, or any custom protocol.

```rust
pub trait Transport: Send + Sync {
    fn send(&mut self, req: Request)
        -> impl Future<Output = Result<Response, TransportError>> + Send;

    fn stream(&mut self, req: Request)
        -> impl Future<Output = Result<EventStream, TransportError>> + Send;
}
```

- **`Request`** — url, headers, body bytes. Providers serialize their wire format into this.
- **`Response`** — status code, headers, body bytes. Providers deserialize from this.
- **`Event`** — `(event_type, data)` tuple. HTTP transports parse SSE; WebSocket transports extract the `type` field from JSON frames. Providers consume these uniformly.
- **`SseParser`** — incremental SSE parser in `transport/sse.rs`, shared by HTTP transports.

`TransportError` variants map to `ChatError` via `From`:
- `Connection` / `Stream` → `ChatError::Network` (retryable)
- `Request` → `ChatError::Provider` (not retryable)

Transport implementations live in separate crates (`transports/reqwest/`, etc.) — core only defines the trait and types.

## Type-State Builder

`ChatBuilder<CP, Output>` uses phantom types to enforce valid configurations at compile time:

- `Unstructured` (default) — `.complete()` returns `ChatResponse`; `.stream()` returns `BoxStream<StreamEvent>`
- `Structured<T>` — `.complete()` returns `StructuredResponse<T>` (T: JsonSchema + DeserializeOwned)
- `Embedded` — `.embed()` returns `EmbeddingsResponse`

State transitions are one-way from `Unstructured`: calling `.with_structured_output::<T>()` or `.with_embeddings()` consumes the builder into the new state.

## Message System

`Messages` wraps `Vec<Content>`. Pushing a `Content` with the same role as the last message merges them (appends parts) rather than creating a new entry.

`Content` has a `role` (User, System, Model) and `parts` containing `PartEnum` variants:

- `Text`, `Reasoning`, `Tool`, `Structured(Value)`, `File`, `Embeddings`

`Tool` is a single part combining a `FunctionCall` with its `ToolStatus` lifecycle — replacing the previous split of `FunctionCall` / `FunctionResponse` parts. Status transitions through: `Pending` → `Approved` / `Rejected` → `Running` → `Completed` / `Failed`. Only resolved states (`Completed`, `Rejected`, `Failed`) ever reach the wire; the rest are local-only and governed by the chat loop's HITL flow.

`File` is either `Url { url, mimetype }` or `Bytes { bytes, mimetype }`.

`Messages::find_tool_mut(id)` locates a `Tool` part anywhere in history by `CallId` — used by HITL callers to resolve a paused tool (`tool.approve(...)` / `tool.reject(...)`).

## Scoped Tool Collections & Strategies

Tools live in `ScopedCollection<M, F>` — a typed `ToolCollection<M>` paired with a strategy closure `F: Fn(&FunctionCall, &M) -> Action`. The metadata type `M` is whatever the user defines via `#[tool(...)]` attributes (e.g. a struct with `requires_approval`, `safety`, rate limits, audit tags).

```rust
let raw = ToolCollection::<ApprovalMeta>::collect_tools()?;
let scoped = ScopedCollection::new(raw, |call, meta| {
    if meta.requires_approval { Action::RequireApproval } else { Action::Execute }
});
let chat = ChatBuilder::new().with_model(client).with_scoped_tools(scoped).build();
```

`Action` variants:

- `Execute` — run the tool immediately.
- `RequireApproval` — mark the tool `Pending`, pause the loop so a human can decide.
- `Schedule { at }` — defer execution; caller persists and resumes later.
- `Reject { reason }` — mark the tool `Rejected` without running it.

A single `Chat` can hold multiple scoped collections with different metadata types. The chat loop dispatches each model-emitted tool call to the collection that owns its name.

## Tool Call Loop

`Chat::tool_call()` runs as a pre-step (before each provider turn) and post-step (after each provider response):

1. Walk the last `Content`'s `Tool` parts.
2. For each `Pending` tool, call the owning collection's strategy closure → `Action`.
3. Execute `Approved`/`Execute` tools; mark results `Completed` / `Failed`.
4. If any tool returned `RequireApproval` or `Schedule`, return a `ToolCallPass` carrying a `PauseReason` and the loop halts.

In streaming mode (`chat::stream.rs`), a pause yields `StreamEvent::Paused(PauseReason)` and terminates the stream. The caller resolves the pending tools on `messages` and calls `chat.stream(&mut messages)` again — the next pre-step runs the newly-approved tools and continues into the next provider turn on the same logical conversational turn. History is preserved across calls via the shared `&mut Messages`.

`PauseReason` variants:

- `AwaitingApproval { tool_ids }`
- `Scheduled { tool_ids, at }`
- `Mixed { ... }` — when a single turn produces more than one pause category

## Error Model

- `ChatError` — enum of error variants (Network, Provider, RateLimited, MaxStepsExceeded, InvalidResponse, Callback, Other)
- `ChatFailure` — wraps `ChatError` with optional `Metadata`, so partial token usage is preserved even on failure
- `TransportError` → `ChatError` conversion via `From`: `Connection`/`Stream` map to `Network` (retryable), `Request` maps to `Provider` (not retryable). Providers use `ChatFailure::from_err` on transport errors to get correct retry semantics automatically.

## Caveats

- `Messages::push()` silently merges consecutive same-role messages. Intentional for tool call flows; can be surprising if you expect separate entries.
- Only resolved `ToolStatus` states (`Completed`, `Rejected`, `Failed`) serialize to the wire via `Tool::to_tuple`. Providers never see `Pending`/`Approved`/`Running`.
- Feature flag `stream` must be enabled at every layer: `chat-core/stream`, and propagated through `chat-rs/stream` to each provider.
- The `Transport` trait lives in `core/src/transport/`. Implementations (e.g., `transport-reqwest`) are separate workspace crates under `transports/`.
- `Transport: Send + Sync` — the `Sync` bound is required because `CompletionProvider: Send + Sync`. WASM transports using non-`Sync` browser types will need the provider trait bounds relaxed first.
- Metadata is provider-specific (`HashMap<String, Value>` on `specific`) — no schema enforcement.
