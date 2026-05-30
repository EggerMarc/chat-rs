# chat-claude

Anthropic Claude provider for [chat-rs](https://github.com/EggerMarc/chat-rs).

## Install

```toml
[dependencies]
chat-core = "0.3.0"
chat-claude = "0.2.3"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Or via the umbrella crate: `chat-rs = { version = "0.4.0", features = ["claude"] }`.

## Usage

```rust
use chat_claude::ClaudeBuilder;
use chat_core::{builder::ChatBuilder, types::messages};

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

## Custom Transport

Supply a custom transport via `.with_transport()` to use something other than the default HTTP (reqwest):

```rust
let client = ClaudeBuilder::new()
    .with_model("claude-sonnet-4-20250514".to_string())
    .with_transport(my_transport)
    .build();
```

## Configuration

- `.with_api_version(version)` — API version (default: `2023-06-01`)
- `.with_thinking_budget(n)` — thinking token budget (default: 10000)

## Feature Flags

Streaming is gated on the `stream` feature:

```toml
chat-claude = { version = "0.2.3", features = ["stream"] }
```
