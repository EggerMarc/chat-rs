# chat-claude

Anthropic Claude provider for [chat-rs](https://github.com/EggerMarc/chat-rs).

## Usage

```rust
use chat_rs::{ChatBuilder, claude::ClaudeBuilder, types::messages};

let client = ClaudeBuilder::new()
    .with_model("claude-sonnet-4-20250514".to_string())
    .build();

let mut chat = ChatBuilder::new().with_model(client).build();

let mut msgs = messages::from_user(vec!["Hello!"]);
let response = chat.complete(&mut msgs).await?;
```

Set `CLAUDE_API_KEY` in your environment or call `.with_api_key()` on the builder.

## Capabilities

- **Completions** — text generation with tool calling and structured output
- **Streaming** — token-by-token output (requires `stream` feature)
- **Extended thinking** — enabled by default, configurable with `.with_thoughts(bool)` and `.with_thinking_budget(u32)`

## Configuration

- `.with_api_version(version)` — API version (default: `2023-06-01`)
- `.with_thinking_budget(n)` — thinking token budget (default: 10000)

## Feature Flags

```toml
chat-rs = { version = "0.0.10", features = ["claude"] }
chat-rs = { version = "0.0.10", features = ["claude", "stream"] }
```
