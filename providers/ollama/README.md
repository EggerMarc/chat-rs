# chat-ollama

[Ollama](https://ollama.com/) provider for [chat-rs](https://github.com/EggerMarc/chat-rs). Thin wrapper over [`chat-completions`](https://crates.io/crates/chat-completions) targeting Ollama's OpenAI-compatible endpoint at `http://localhost:11434/v1`, with extras (`.pull()` against the native `/api/pull`).

## Usage

```rust
use chat_rs::{ChatBuilder, ollama::OllamaBuilder, types::messages};

let client = OllamaBuilder::new()
    .with_model("llama3.2")
    .build();

let mut chat = ChatBuilder::new().with_model(client).build();

let mut msgs = messages::from_user(vec!["Hello!"]);
let response = chat.complete(&mut msgs).await?;
```

No API key required by default. Honors `OLLAMA_HOST` if you point at a remote daemon.

## Capabilities

- **Completions** — text generation with tool calling and structured output
- **Streaming** — token-by-token output (requires `stream` feature)
- **Embeddings** — vector embeddings via `.with_embeddings()`
- **Model management** — `.pull()` calls Ollama's native `/api/pull` to fetch a model if missing

## Pull and Build

```rust
let client = OllamaBuilder::new()
    .with_model("llama3.2")
    .pull().await?   // downloads the model if not present
    .build();
```

## Custom Endpoint / Transport

Override the base URL with `.with_base_url(...)` or supply a custom transport with `.with_transport(...)`. `OLLAMA_HOST` is read automatically if set.

## Feature Flags

```toml
chat-rs = { version = "0.2.1", features = ["ollama"] }
chat-rs = { version = "0.2.1", features = ["ollama", "stream"] }
```
