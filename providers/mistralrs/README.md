# chat-mistralrs

Local-inference provider for [chat-rs](https://github.com/EggerMarc/chat-rs), built on [mistral.rs](https://github.com/EricLBuehler/mistral.rs). Loads weights in-process — no HTTP, no daemon. Geared at local multimodal/agentic workflows: Qwen2.5-VL and similar text/image/audio models, structured outputs, and tool calling.

## Install

```toml
[dependencies]
chat-core = "0.3.0"
chat-mistralrs = "0.1.4"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Or via the umbrella crate: `chat-rs = { version = "0.4.0", features = ["mistralrs"] }`.

## Usage

```rust
use chat_mistralrs::MistralRsBuilder;
use chat_core::{builder::ChatBuilder, types::messages};

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
use chat_mistralrs::{DeviceChoice, MistralRsBuilder};

let client = MistralRsBuilder::new()
    .with_model("...")
    .with_device(DeviceChoice::Auto)  // or specific Cpu / Cuda / Metal
    .build()
    .await?;
```

## Versioning

Tracks the latest mistral.rs release without pinning; upstream churn is treated as normal maintenance.

## Feature Flags

Streaming is gated on the `stream` feature:

```toml
chat-mistralrs = { version = "0.1.4", features = ["stream"] }
```
