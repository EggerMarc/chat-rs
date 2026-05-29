# chat-huggingface

[Hugging Face Inference Providers (Router)](https://huggingface.co/docs/inference-providers/) provider for [chat-rs](https://github.com/EggerMarc/chat-rs). Thin wrapper over [`chat-completions`](https://crates.io/crates/chat-completions) targeting `https://router.huggingface.co/v1`. Routes any HF-hosted model through a single OpenAI-compatible endpoint, with provider selection via the model slug (`:fastest`, `:cheapest`, or a specific provider).

## Install

```toml
[dependencies]
chat-core = "0.3.0"
chat-huggingface = "0.2.3"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Or via the umbrella crate: `chat-rs = { version = "0.4.0", features = ["huggingface"] }`.

## Usage

```rust
use chat_huggingface::HuggingFaceBuilder;
use chat_core::{builder::ChatBuilder, types::messages};

let client = HuggingFaceBuilder::new()
    .with_model("openai/gpt-oss-120b:fastest")
    .build();

let mut chat = ChatBuilder::new().with_model(client).build();

let mut msgs = messages::from_user(vec!["Hello!"]);
let response = chat.complete(&mut msgs).await?;
```

Set `HF_TOKEN` in your environment or call `.with_api_key()` on the builder.

## Capabilities

- **Completions** — text generation with tool calling and structured output
- **Streaming** — token-by-token output (requires `stream` feature)

## Custom Endpoint / Transport

Override the base URL with `.with_base_url(...)` or supply a custom transport with `.with_transport(...)`.

## Feature Flags

Streaming is gated on the `stream` feature:

```toml
chat-huggingface = { version = "0.2.3", features = ["stream"] }
```
