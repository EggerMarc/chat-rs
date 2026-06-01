# chat-deepseek

[DeepSeek](https://api-docs.deepseek.com/) provider for [chat-rs](https://github.com/EggerMarc/chat-rs). Thin wrapper over [`chat-completions`](https://crates.io/crates/chat-completions) targeting DeepSeek's OpenAI-compatible `/chat/completions` endpoint.

## Install

```toml
[dependencies]
chat-core = "0.4.0"
chat-deepseek = "0.1.3"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Or via the umbrella crate: `chat-rs = { version = "0.5.0", features = ["deepseek"] }`.

## Usage

```rust
use chat_deepseek::DeepSeekBuilder;
use chat_core::{builder::ChatBuilder, types::messages};

let client = DeepSeekBuilder::new()
    .with_model("deepseek-v4-pro")
    .build();

let mut chat = ChatBuilder::new().with_model(client).build();

let mut msgs = messages::from_user(vec!["Hello!"]);
let response = chat.complete(&mut msgs).await?;
```

Set `DEEPSEEK_API_KEY` in your environment or call `.with_api_key()` on the builder. Models: `deepseek-v4-pro`, `deepseek-v4-flash` (legacy `deepseek-chat` / `deepseek-reasoner` are deprecated).

## Capabilities

- **Completions** — text generation with tool calling and structured output
- **Streaming** — token-by-token output (requires `stream` feature)

## Note on DSML / Tool-Call Format

DeepSeek V4 open-weight checkpoints natively emit DSML-tagged (XML-ish) tool calls (`<｜DSML｜tool_calls>` / `<｜DSML｜invoke>` / `<｜DSML｜parameter>`). The hosted API parses that into standard OpenAI-shape JSON `tool_calls` before returning, so this client sees only JSON on the wire. A local DeepSeek runtime (e.g. via `chat-mistralrs`) would need its own DSML parser at the inference layer — out of scope for this crate.

## Custom Endpoint / Transport

Override the base URL with `.with_base_url(...)` or supply a custom transport with `.with_transport(...)`.

## Feature Flags

Streaming is gated on the `stream` feature:

```toml
chat-deepseek = { version = "0.1.2", features = ["stream"] }
```
