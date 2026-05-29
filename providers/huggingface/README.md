# chat-huggingface

[Hugging Face Inference Providers (Router)](https://huggingface.co/docs/inference-providers/) provider for [chat-rs](https://github.com/EggerMarc/chat-rs). Thin wrapper over [`chat-completions`](https://crates.io/crates/chat-completions) targeting `https://router.huggingface.co/v1`. Routes any HF-hosted model through a single OpenAI-compatible endpoint, with provider selection via the model slug (`:fastest`, `:cheapest`, or a specific provider).

## Usage

```rust
use chat_rs::{ChatBuilder, huggingface::HuggingFaceBuilder, types::messages};

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

```toml
chat-rs = { version = "0.2.1", features = ["huggingface"] }
chat-rs = { version = "0.2.1", features = ["huggingface", "stream"] }
```
