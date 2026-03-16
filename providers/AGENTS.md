# Providers

All provider crates live under `providers/`. Each implements the core traits (`CompletionProvider`, optionally `StreamProvider` and `EmbeddingsProvider`) to bridge `chat-core` types to a specific LLM API.

## Current Providers

| Crate | Directory | API Key Env Var | API Style |
|---|---|---|---|
| `chat-gemini` | `providers/gemini` | `GEMINI_API_KEY` | Gemini `generateContent` / `embedContent` |
| `chat-openai` | `providers/openai` | `OPENAI_API_KEY` | OpenAI Responses API (`/v1/responses`) |

## Common Architecture

Every provider follows the same structure:

```
providers/<name>/src/
├── lib.rs              # Builder (type-state pattern) + public exports
├── client.rs           # Client struct (holds HTTP client, config, native tools)
├── api/
│   ├── mod.rs
│   ├── completion.rs   # impl CompletionProvider
│   ├── embedding.rs    # impl EmbeddingsProvider
│   ├── stream.rs       # impl StreamProvider (feature-gated)
│   └── types/
│       ├── request.rs  # Core Messages -> provider API request
│       ├── response.rs # Provider API response -> core ChatResponse
│       └── error.rs    # HTTP error handling
└── tools/
    ├── mod.rs          # NativeTool trait definition
    └── <tool>.rs       # Individual native tool implementations
```

## Common Patterns

### Type-State Builders
Each provider has a builder with phantom type parameters that enforce valid configurations at compile time. You cannot call `.build()` without first setting a model. Calling completion-specific methods (like native tools) transitions the builder from `BaseConfig` to `CompletionConfig`, preventing mixing with `EmbeddingConfig`.

### Native Tools
Each provider defines its own `<Provider>NativeTool` trait:
```rust
pub trait NativeTool: Send + Sync {
    fn tool_key(&self) -> &'static str;
    fn is_search(&self) -> bool;
    fn to_tool_declaration(&self) -> Value;
    fn to_tool_config(&self) -> Option<(String, Value)>;
    fn clone_box(&self) -> Box<dyn NativeTool>;
}
```
Native tools are provider-specific API features (e.g., Google Search for Gemini, Web Search for OpenAI) — distinct from user-defined function tools registered via `ToolCollection`.

### Request/Response Transformation
Each provider implements a `from_core()` function to convert `Messages` + options into the provider's API request format, and an `into_core_chat_response()` to convert the API response back to `ChatResponse`.

## Adding a New Provider

### 1. Create the crate

```
mkdir -p providers/new-provider/src/{api/types,tools}
```

### 2. Set up `Cargo.toml`

```toml
[package]
name = "chat-new-provider"
version = "0.0.1"
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
chat-core = { path = "../../core", version = "0.0.3" }
async-trait.workspace = true
reqwest = { workspace = true, features = ["json", "stream"] }
serde.workspace = true
serde_json.workspace = true
tools-rs.workspace = true
schemars.workspace = true

[features]
default = []
stream = ["chat-core/stream", "dep:async-stream", "dep:futures"]
```

### 3. Implement the client struct (`client.rs`)

```rust
pub struct NewProviderClient {
    pub(crate) model_name: String,
    pub(crate) api_key: String,
    pub(crate) http_client: reqwest::Client,
    pub(crate) native_tools: Vec<Box<dyn NewProviderNativeTool>>,
}
```

### 4. Implement `CompletionProvider` (`api/completion.rs`)

This is the minimum required trait. Transform core `Messages` to your API's request format, make the HTTP call, and transform the response back to `ChatResponse`.

Key responsibilities:
- Map `PartEnum` variants to the API's content format
- Handle system messages (some APIs use a separate field, others inline them)
- Serialize `ToolCollection` schemas into the API's tool format
- If `structured_output` is `Some`, configure the API for JSON schema output
- Parse the response into `Content` with correct `RoleEnum` and `CompleteReasonEnum`
- Build `Metadata` with token usage

### 5. Implement `EmbeddingsProvider` (`api/embedding.rs`) — optional

Extract text from messages, call the embeddings endpoint, return `EmbeddingsResponse`.

### 6. Implement `StreamProvider` (`api/stream.rs`) — optional, feature-gated

Return a `BoxStream<Result<StreamEvent, ChatError>>`. Most APIs use SSE; you'll need to parse the event stream and yield `StreamEvent` variants (`TextChunk`, `ReasoningChunk`, `ToolCall`, `Done`).

Implement `on_stream_done()` if you need to store state from the final response (e.g., OpenAI stores `last_response_id`).

### 7. Build the type-state builder (`lib.rs`)

Follow the existing pattern: phantom types for `WithoutModel`/`WithModel` and `BaseConfig`/`CompletionConfig`/`EmbeddingConfig`. The builder must enforce that `.build()` requires a model.

### 8. Register in workspace

**Root `Cargo.toml`:**
```toml
# Add to [workspace] members
members = ["core", "providers/gemini", "providers/openai", "providers/new-provider"]

# Add to [dependencies]
chat-new-provider = { path = "providers/new-provider", optional = true, version = "0.0.1" }

# Add to [features]
new-provider = ["dep:chat-new-provider"]
stream = ["chat-core/stream", "chat-gemini?/stream", "chat-openai?/stream", "chat-new-provider?/stream"]
```

**Root `src/lib.rs`:**
```rust
#[cfg(feature = "new-provider")]
pub mod new_provider {
    pub use chat_new_provider::*;
}
```

Add to the `prelude` module as well.

## Caveats

- The OpenAI provider uses the **Responses API** (`/v1/responses`), not the Chat Completions API. Custom endpoints (via `.with_custom_url()`) must support this format.
- Provider-specific builder options (e.g., `reasoning_effort`, `with_thoughts`) emit warnings when used with custom endpoints that may not support them.
- Each provider's API key defaults to an environment variable (`GEMINI_API_KEY`, `OPENAI_API_KEY`) and panics if not found. Pass the key explicitly via `.with_api_key()` to avoid this.
- The `metadata.specific` field in responses is a `HashMap<String, Value>` — providers can put arbitrary data here without schema guarantees.
