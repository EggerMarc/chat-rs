# chat-responses

Generic OpenAI Responses API wire client for [chat-rs](https://github.com/EggerMarc/chat-rs). Symmetric counterpart to [`chat-completions`](https://crates.io/crates/chat-completions): owns the `/responses` endpoint wire types and SSE stream parsing. Point it at any server implementing the Responses API.

## Install

```toml
[dependencies]
chat-core = "0.2.1"
chat-responses = "0.1.0"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Or via the umbrella crate: `chat-rs = { version = "0.3.0", features = ["responses"] }`.

## Usage

```rust
use chat_responses::ResponsesBuilder;
use chat_core::{builder::ChatBuilder, types::messages};

let client = ResponsesBuilder::new()
    .with_base_url("https://api.openai.com/v1")
    .with_model("gpt-4o")
    .with_api_key("sk-...")
    .build();

let mut chat = ChatBuilder::new().with_model(client).build();

let mut msgs = messages::from_user(vec!["Hello!"]);
let response = chat.complete(&mut msgs).await?;
```

## Capabilities

- **Completions** — text generation with tool calling and structured output
- **Streaming** — SSE token streaming (requires `stream` feature)
- **Reasoning effort** — `.with_reasoning_effort("medium")` for reasoning models
- **Previous-response-id round-tripping** — server-side context resume (default on)
- **Custom tool declarations** — `.with_tool_declaration(value)` for wrappers that want to inject provider-specific tools as raw JSON

## When to Use This vs `chat-openai`

Use `chat-responses` directly when you control the endpoint or are wrapping a Responses-API provider that isn't OpenAI. Use [`chat-openai`](https://crates.io/crates/chat-openai) when you want the OpenAI defaults (URL + `OPENAI_API_KEY`), the OpenAI-specific native tools (`web_search`, `image_generation`), and the `/embeddings` endpoint.

## Decoupled Native Tools

The wire layer is trait-agnostic. Wrappers materialize their provider-specific native tools into `serde_json::Value` and pass them via `.with_tool_declaration()`:

```rust
let client = ResponsesBuilder::new()
    .with_base_url("https://api.openai.com/v1")
    .with_model("gpt-4o")
    .with_api_key("sk-...")
    .with_tool_declaration(serde_json::json!({ "type": "web_search" }))
    .build();
```

## Custom Transport

Pluggable via `.with_transport()`. Defaults to HTTP/SSE via reqwest. WebSocket transports (`AsyncWsTransport`, `WsTransport`) work when paired with a Responses-API WS endpoint.

## Feature Flags

Streaming is gated on the `stream` feature:

```toml
chat-responses = { version = "0.1.0", features = ["stream"] }
```
