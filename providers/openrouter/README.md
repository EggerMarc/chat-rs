# chat-openrouter

[OpenRouter](https://openrouter.ai/) provider for [chat-rs](https://github.com/EggerMarc/chat-rs). Thin wrapper over [`chat-responses`](https://crates.io/crates/chat-responses) targeting OpenRouter's OpenAI-compatible **Responses API (Beta)** at `https://openrouter.ai/api/v1/responses`.

OpenRouter is a unified gateway in front of hundreds of models from many vendors. The model slug selects which one, and is vendor-prefixed (e.g. `anthropic/claude-sonnet-4`, `openai/gpt-4o`, `google/gemini-2.5-pro`).

## Install

```toml
[dependencies]
chat-core = "0.4.0"
chat-openrouter = "0.1.0"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Or via the umbrella crate: `chat-rs = { version = "0.5.0", features = ["openrouter"] }`.

## Usage

```rust
use chat_openrouter::OpenRouterBuilder;
use chat_core::{builder::ChatBuilder, types::messages};

let client = OpenRouterBuilder::new()
    .with_model("anthropic/claude-sonnet-4")
    .build();

let mut chat = ChatBuilder::new().with_model(client).build();

let mut msgs = messages::from_user(vec!["Hello!"]);
let response = chat.complete(&mut msgs).await?;
```

Set `OPENROUTER_API_KEY` in your environment or call `.with_api_key()` on the builder.

## Capabilities

- **Completions** — text generation with tool calling and structured output
- **Streaming** — token-by-token output over SSE (requires the `stream` feature)
- **Reasoning** — `.with_reasoning_effort("high")` for reasoning-capable models

## Notes

- **Wire spec.** OpenRouter speaks the OpenAI Responses API, so this crate wraps `chat-responses` (the Responses API wire client) rather than `chat-completions` — the same way `chat-openai` does.
- **Stateless.** The OpenRouter Responses API persists no server-side state and has no `previous_response_id` round-trip. The builder always disables response-id reuse and sends the full conversation each turn.
- **No WebSocket.** OpenRouter has no WebSocket/realtime endpoint; streaming is SSE over HTTP. The builder is still generic over `Transport`, so a custom transport can be supplied via `.with_transport(...)`.

## Custom Endpoint / Transport

Override the base URL with `.with_base_url(...)` or supply a custom transport with `.with_transport(...)`.

## Feature Flags

Streaming is gated on the `stream` feature:

```toml
chat-openrouter = { version = "0.1.0", features = ["stream"] }
```
