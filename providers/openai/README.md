# chat-openai

OpenAI provider for [chat-rs](https://github.com/EggerMarc/chat-rs). Thin wrapper over [`chat-responses`](https://crates.io/crates/chat-responses) — presets `https://api.openai.com/v1` and `OPENAI_API_KEY`, adds the OpenAI-specific native tools (`web_search`, `image_generation`) and the `/embeddings` endpoint (which is **not** part of the Responses API).

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

- **Completions** — text generation with tool calling and structured output, via the Responses API
- **Streaming** — SSE token streaming (requires `stream` feature)
- **Embeddings** — vector embeddings via `.with_embeddings()` (uses OpenAI's `/embeddings`, not Responses)
- **Reasoning effort** — `.with_reasoning_effort(effort)` for o1/o3/GPT-5 reasoning models
- **Previous-response-id round-tripping** — server-side context resume (default on; disable with `.without_previous_response_id()`)

## Native Tools

- **Web Search** — `.with_web_search(context_size, user_location)`
- **Image Generation** — `.with_image_generation(tool)`

## Transport

The default transport is HTTP/SSE via reqwest. To use WebSocket streaming:

```rust
use chat_rs::{openai::OpenAIBuilder, transport::AsyncWsTransport};

let ws = AsyncWsTransport::new()
    .with_message_type("response.create");

let client = OpenAIBuilder::new()
    .with_model("gpt-4o")
    .with_transport(ws)
    .build();
```

Requires the `tokio-tungstenite` feature. A sync variant (`WsTransport`) is available via the `tungstenite` feature.

You can also supply any custom `Transport` implementation via `.with_transport()`.

## Custom Endpoints

Point at local or proxy servers implementing the Responses API:

```rust
let client = OpenAIBuilder::new()
    .with_model("custom-model")
    .with_custom_url("https://my-gateway/v1".to_string())
    .with_api_key("...".to_string())
    .build();
```

For full direct control of the Responses wire (no OpenAI defaults), use [`chat-responses`](https://crates.io/crates/chat-responses) directly.

## Feature Flags

```toml
chat-rs = { version = "0.2.1", features = ["openai"] }
chat-rs = { version = "0.2.1", features = ["openai", "stream"] }
chat-rs = { version = "0.2.1", features = ["openai", "stream", "tokio-tungstenite"] }
```
