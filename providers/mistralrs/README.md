# chat-mistralrs

Local-inference provider for [chat-rs](https://github.com/EggerMarc/chat-rs), built on [mistral.rs](https://github.com/EricLBuehler/mistral.rs). Loads weights in-process — no HTTP, no daemon. Geared at local multimodal/agentic workflows: Qwen2.5-VL and similar text/image/audio models, structured outputs, and tool calling.

## Usage

```rust
use chat_rs::{ChatBuilder, mistralrs::MistralRsBuilder, types::messages};

let client = MistralRsBuilder::new()
    .with_model("Qwen/Qwen2.5-3B-Instruct-GGUF")
    .with_gguf_file("qwen2.5-3b-instruct-q4_k_m.gguf")
    .build()
    .await?;

let mut chat = ChatBuilder::new().with_model(client).build();

let mut msgs = messages::from_user(vec!["Hello!"]);
let response = chat.complete(&mut msgs).await?;
```

On first use, model files are fetched into the standard Hugging Face cache (`~/.cache/huggingface/`). Set `HF_TOKEN` for gated repos.

## Capabilities

- **Completions** — text, image, and audio inputs work today
- **Streaming** — token-by-token output (requires `stream` feature)
- **Tool calling & structured outputs** — planned

## Device Selection

```rust
use chat_rs::mistralrs::DeviceChoice;

let client = MistralRsBuilder::new()
    .with_model("...")
    .with_device(DeviceChoice::Auto)  // or specific Cpu / Cuda / Metal
    .build()
    .await?;
```

## Versioning

Tracks the latest mistral.rs release without pinning; upstream churn is treated as normal maintenance.

## Feature Flags

```toml
chat-rs = { version = "0.2.1", features = ["mistralrs"] }
chat-rs = { version = "0.2.1", features = ["mistralrs", "stream"] }
```
