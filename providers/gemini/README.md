# chat-gemini

Google Gemini provider for [chat-rs](https://github.com/EggerMarc/chat-rs).

## Usage

```rust
use chat_rs::{ChatBuilder, gemini::GeminiBuilder, types::messages};

let client = GeminiBuilder::new()
    .with_model("gemini-2.5-flash".to_string())
    .build();

let mut chat = ChatBuilder::new().with_model(client).build();

let mut msgs = messages::from_user(vec!["Hello!"]);
let response = chat.complete(&mut msgs).await?;
```

Set `GEMINI_API_KEY` in your environment or call `.with_api_key()` on the builder.

## Capabilities

- **Completions** — text generation with tool calling and structured output
- **Streaming** — token-by-token output (requires `stream` feature)
- **Embeddings** — vector embeddings via `.with_embeddings(dimensions)`
- **Extended thinking** — enable with `.with_thoughts(true)`

## Custom Transport

Supply a custom transport via `.with_transport()` to use something other than the default HTTP (reqwest):

```rust
let client = GeminiBuilder::new()
    .with_model("gemini-2.5-flash".to_string())
    .with_transport(my_transport)
    .build();
```

## Native Tools

- **Google Search** — `.with_google_search()` or `.with_google_search_threshold(f32)`
- **Code Execution** — `.with_code_execution()`
- **Google Maps** — `.with_google_maps(lat_lng, widget)`

## Feature Flags

```toml
chat-rs = { version = "0.1.0", features = ["gemini"] }
chat-rs = { version = "0.1.0", features = ["gemini", "stream"] }
```
