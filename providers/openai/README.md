# chat-openai

OpenAI provider for [chat-rs](https://github.com/EggerMarc/chat-rs). Uses the Responses API.

## Usage

```rust
use chat_rs::{ChatBuilder, openai::OpenAIBuilder, types::messages};

let client = OpenAIBuilder::new()
    .with_model("gpt-4o")
    .build();

let mut chat = ChatBuilder::new().with_model(client).build();

let mut msgs = messages::from_user(vec!["Hello!"]);
let response = chat.complete(&mut msgs).await?;
```

Set `OPENAI_API_KEY` in your environment or call `.with_api_key()` on the builder.

## Capabilities

- **Completions** — text generation with tool calling and structured output
- **Streaming** — token-by-token output (requires `stream` feature)
- **Embeddings** — vector embeddings via `.with_embeddings()`
- **Reasoning effort** — `.with_reasoning_effort(effort)` for o1/o3 models

## Native Tools

- **Web Search** — `.with_web_search(context_size, user_location)`

## Custom Transport

Supply a custom transport via `.with_transport()` to use something other than the default HTTP (reqwest):

```rust
let client = OpenAIBuilder::new()
    .with_model("gpt-4o")
    .with_transport(my_transport)
    .build();
```

## Custom Endpoints

Use local or proxy servers that implement the Responses API:

```rust
let client = OpenAIBuilder::new()
    .with_model("llama3")
    .with_custom_url("http://localhost:11434/v1".to_string())
    .with_api_key("ollama".to_string())
    .build();
```

## Feature Flags

```toml
chat-rs = { version = "0.0.16", features = ["openai"] }
chat-rs = { version = "0.0.16", features = ["openai", "stream"] }
```
