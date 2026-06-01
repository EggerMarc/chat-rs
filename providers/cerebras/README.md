# chat-cerebras

[Cerebras Inference](https://inference.cerebras.ai/) provider for [chat-rs](https://github.com/EggerMarc/chat-rs). Thin wrapper over [`chat-completions`](https://crates.io/crates/chat-completions) targeting Cerebras's OpenAI-compatible `/v1/chat/completions` endpoint — wafer-scale silicon means extremely fast token throughput.

## Install

```toml
[dependencies]
chat-core = "0.4.0"
chat-cerebras = "0.2.4"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Or via the umbrella crate: `chat-rs = { version = "0.5.0", features = ["cerebras"] }`.

## Usage

```rust
use chat_cerebras::CerebrasBuilder;
use chat_core::{builder::ChatBuilder, types::messages};

let client = CerebrasBuilder::new()
    .with_model("llama-3.3-70b")
    .build();

let mut chat = ChatBuilder::new().with_model(client).build();

let mut msgs = messages::from_user(vec!["Hello!"]);
let response = chat.complete(&mut msgs).await?;
```

Set `CEREBRAS_API_KEY` in your environment or call `.with_api_key()` on the builder.

## Capabilities

- **Completions** — text generation with tool calling and structured output
- **Streaming** — token-by-token output (requires `stream` feature)

## Custom Endpoint / Transport

Override the base URL with `.with_base_url(...)` or supply a custom transport with `.with_transport(...)`.

## Feature Flags

Streaming is gated on the `stream` feature of `chat-cerebras` (or on `chat-rs` if using the umbrella):

```toml
chat-cerebras = { version = "0.2.3", features = ["stream"] }
```
