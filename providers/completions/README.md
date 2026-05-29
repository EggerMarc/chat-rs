# chat-completions

Generic OpenAI Chat Completions wire client for [chat-rs](https://github.com/EggerMarc/chat-rs). Targets the `/v1/chat/completions` wire spec — point it at any OAI-compatible server (vLLM, llama.cpp, LiteLLM, Together, Fireworks, your own gateway).

## Install

```toml
[dependencies]
chat-core = "0.3.0"
chat-completions = "0.2.3"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Or via the umbrella crate: `chat-rs = { version = "0.4.0", features = ["completions"] }`.

## Usage

```rust
use chat_completions::ChatCompletionsBuilder;
use chat_core::{builder::ChatBuilder, types::messages};

let client = ChatCompletionsBuilder::new()
    .with_base_url("http://localhost:8000/v1")
    .with_model("my-model")
    .with_api_key("sk-...")  // optional — omit for servers that don't require auth
    .build();

let mut chat = ChatBuilder::new().with_model(client).build();

let mut msgs = messages::from_user(vec!["Hello!"]);
let response = chat.complete(&mut msgs).await?;
```

Bring your own base URL, model name, and (optional) API key.

## Capabilities

- **Completions** — text generation with tool calling and structured output
- **Streaming** — token-by-token output (requires `stream` feature)
- **Embeddings** — vector embeddings (where the server supports `/embeddings`)

## When to Use This vs a Dedicated Wrapper

Use `chat-completions` directly when you control the endpoint or want maximum flexibility. Use a dedicated wrapper (`chat-ollama`, `chat-huggingface`, `chat-cerebras`, `chat-deepseek`) when you want preset URLs, env var conventions, and provider-specific niceties.

## Custom Transport

Supply a custom transport via `.with_transport()` to use something other than the default HTTP (reqwest):

```rust
let client = ChatCompletionsBuilder::new()
    .with_base_url("http://localhost:8000/v1")
    .with_model("my-model")
    .with_transport(my_transport)
    .build();
```

## Feature Flags

Streaming is gated on the `stream` feature:

```toml
chat-completions = { version = "0.2.3", features = ["stream"] }
```
