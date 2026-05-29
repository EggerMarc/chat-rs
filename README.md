# chat-rs

A multi-provider LLM framework for Rust. Build type-safe chat clients with tool calling, structured output, streaming, and embeddings — swap providers with a single line change.

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.94%2B-orange.svg)](https://www.rust-lang.org)

## Features

- **Multi-provider** — Gemini, Claude, OpenAI, DeepSeek, Ollama, Hugging Face, Cerebras, mistral.rs (local), generic OpenAI-compatible servers, generic Responses API servers, and Router today, more coming (see [Roadmap](ROADMAP.md))
- **Router** — route requests across multiple providers with fallback and custom strategies (keyword, embedding, capability-based)
- **Type-safe builder** — compile-time enforcement of valid configurations via type-state pattern
- **Tool calling** — define tools with `#[tool]` in Rust, or load `@tool`-decorated Python scripts at runtime; the framework handles the call loop automatically
- **Structured output** — deserialize model responses directly into your Rust types via `schemars`
- **Streaming** — real-time token-by-token output with tool call support
- **Human in the loop** — pause mid-turn on sensitive tool calls, let a human approve or reject, then resume the stream
- **Embeddings** — generate vector embeddings through the same unified API
- **Retry & callbacks** — configurable retry strategies with before/after hooks
- **Native tools** — provider-specific features like Google Search, code execution, web search

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
chat-rs = { version = "0.3.0", features = ["openai"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use chat_rs::{ChatBuilder, openai::OpenAIBuilder, types::messages};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = OpenAIBuilder::new().with_model("gpt-4o-mini").build();
    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::from_user(vec!["Hey there!"]);
    let res = chat.complete(&mut messages).await?;
    println!("{:?}", res.content);

    Ok(())
}
```

Set your API key via environment variable (`OPENAI_API_KEY`, `GEMINI_API_KEY`, or `CLAUDE_API_KEY`), or pass it explicitly with `.with_api_key()`.

## Providers

Enable providers via feature flags:

```toml
# Pick one or more
chat-rs = { version = "0.3.0", features = ["gemini"] }
chat-rs = { version = "0.3.0", features = ["claude"] }
chat-rs = { version = "0.3.0", features = ["openai"] }
chat-rs = { version = "0.3.0", features = ["ollama"] }
chat-rs = { version = "0.3.0", features = ["huggingface"] }
chat-rs = { version = "0.3.0", features = ["cerebras"] }
chat-rs = { version = "0.3.0", features = ["completions"] }
chat-rs = { version = "0.3.0", features = ["router", "gemini", "claude"] }
chat-rs = { version = "0.3.0", features = ["gemini", "claude", "openai", "stream"] }
```

| Provider | Feature | API Key Env Var | Builder |
|---|---|---|---|
| Google Gemini | `gemini` | `GEMINI_API_KEY` | `GeminiBuilder` |
| Anthropic Claude | `claude` | `CLAUDE_API_KEY` | `ClaudeBuilder` |
| OpenAI | `openai` | `OPENAI_API_KEY` | `OpenAIBuilder` |
| DeepSeek | `deepseek` | `DEEPSEEK_API_KEY` | `DeepSeekBuilder` |
| Ollama (local) | `ollama` | — (optional) | `OllamaBuilder` |
| Hugging Face Router | `huggingface` | `HF_TOKEN` | `HuggingFaceBuilder` |
| Cerebras | `cerebras` | `CEREBRAS_API_KEY` | `CerebrasBuilder` |
| mistral.rs (local in-process) | `mistralrs` | — | `MistralRsBuilder` |
| Generic Chat Completions | `completions` | depends on server | `ChatCompletionsBuilder` |
| Generic Responses API | `responses` | depends on server | `ResponsesBuilder` |
| Router | `router` | — | `RouterBuilder` |

The `ollama`, `huggingface`, `cerebras`, `deepseek`, and `completions` providers all share the same Chat Completions wire spec, factored into the `chat-completions` crate. The `openai` provider is a thin wrapper over `chat-responses` (the Responses API wire crate). Bring-your-own server: use `ChatCompletionsBuilder` for `/v1/chat/completions` servers (vLLM, llama.cpp, LiteLLM, etc.) or `ResponsesBuilder` for `/responses` servers.

For fully **local in-process** inference (no HTTP, no daemon), use the `mistralrs` provider — weights load into your process via [mistral.rs](https://github.com/EricLBuehler/mistral.rs).

Swapping providers is a one-line change — replace the builder, everything else stays the same:

```rust
// Gemini
let client = GeminiBuilder::new()
    .with_model("gemini-2.5-flash".to_string())
    .build();

// Claude
let client = ClaudeBuilder::new()
    .with_model("claude-sonnet-4-20250514".to_string())
    .build();

// OpenAI
let client = OpenAIBuilder::new()
    .with_model("gpt-4o")
    .build();

// Ollama (local) — pulls the model if missing, then builds
let client = OllamaBuilder::new()
    .with_model("llama3.2")
    .pull().await?
    .build();

// Hugging Face Inference Providers
let client = HuggingFaceBuilder::new()
    .with_model("openai/gpt-oss-120b:fastest")
    .build();

// Cerebras
let client = CerebrasBuilder::new()
    .with_model("llama-3.3-70b")
    .build();

// DeepSeek
let client = DeepSeekBuilder::new()
    .with_model("deepseek-v4-pro")
    .build();

// mistral.rs (local, in-process — no HTTP)
let client = MistralRsBuilder::new()
    .with_model("Qwen/Qwen2.5-3B-Instruct-GGUF")
    .with_gguf_file("qwen2.5-3b-instruct-q4_k_m.gguf")
    .build().await?;

// Bring-your-own Chat Completions server (vLLM, llama.cpp, LiteLLM, ...)
let client = ChatCompletionsBuilder::new()
    .with_base_url("http://localhost:8000/v1")
    .with_model("my-model")
    .with_api_key("sk-...")
    .build();

// Bring-your-own Responses API server
let client = ResponsesBuilder::new()
    .with_base_url("https://your-gateway/v1")
    .with_model("my-model")
    .with_api_key("sk-...")
    .build();

// Same from here on
let mut chat = ChatBuilder::new().with_model(client).build();
```

## Tool Calling

Define tools with the `#[tool]` macro from `tools-rs` and register them with `collect_tools()`. The framework automatically loops through tool calls until the model is done.

```rust
use chat_rs::{ChatBuilder, gemini::GeminiBuilder, types::messages::content};
use tools_rs::{collect_tools, tool};

#[tool]
/// Looks up the current weather for a given city.
async fn get_weather(city: String) -> String {
    format!("The weather in {} is sunny, 22°C", city)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        .build();

    let tools = collect_tools();

    let mut chat = ChatBuilder::new()
        .with_tools(tools)
        .with_model(client)
        .with_max_steps(5)
        .build();

    let mut messages = messages::Messages::default();
    messages.push(content::from_user(vec!["What's the weather in Tokyo?"]));

    let response = chat.complete(&mut messages).await.map_err(|e| e.err)?;
    println!("{:?}", response.content);

    Ok(())
}
```

### Python Tools

Load tools from Python scripts at runtime via the `python` feature (powered by `tools-rs` 0.3 + PyO3). Decorate functions with `@tool()` and point `ToolsBuilder` at a directory of `.py` files — they register alongside any native `#[tool]`s.

```toml
chat-rs = { version = "0.3.0", features = ["gemini", "python"] }
```

```python
# scripts/weather.py
from tools_rs import tool

@tool()
def get_weather(city: str) -> str:
    """Get the current weather in a city.

    Args:
        city: The city to look up.
    """
    return {"London": "rainy, 12C", "Tokyo": "sunny, 22C"}.get(city, "unknown")
```

```rust
use tools_rs::{Language, ToolsBuilder};

let tools = ToolsBuilder::new()
    .with_language(Language::Python)
    .from_path("scripts")
    .collect()?;

let mut chat = ChatBuilder::new()
    .with_tools(tools)
    .with_model(client)
    .build();
```

PyO3 builds against the system Python; if your interpreter is newer than PyO3's max supported version, set `PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1` when building.

## Structured Output

Deserialize model responses directly into typed Rust structs. Your type must derive `JsonSchema` and `Deserialize`.

```rust
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(JsonSchema, Deserialize, Clone, Debug)]
struct User {
    pub name: String,
    pub likes: Vec<String>,
}

let mut chat = ChatBuilder::new()
    .with_structured_output::<User>()
    .with_model(client)
    .build();

let response = chat.complete(&mut messages).await?;
println!("Name: {}, Likes: {:?}", response.content.name, response.content.likes);
```

## Streaming

Enable the `stream` feature flag:

```toml
chat-rs = { version = "0.3.0", features = ["gemini", "stream"] }
```

```rust
use chat_rs::StreamEvent;
use futures::StreamExt;

let mut chat = ChatBuilder::new()
    .with_model(client)
    .build();

let mut stream = chat.stream(&mut messages).await?;

while let Some(chunk) = stream.next().await {
    match chunk? {
        StreamEvent::TextChunk(text) => print!("{}", text),
        StreamEvent::ReasoningChunk(thought) => print!("[thinking] {}", thought),
        StreamEvent::ToolCall(fc) => println!("[calling {}]", fc.name),
        StreamEvent::ToolResult(fr) => println!("[tool returned]"),
        StreamEvent::Done(_) => break,
    }
}
```

## Human in the Loop

Mark tools that need human approval via `#[tool]` metadata and supply a strategy closure. When the model calls such a tool, `chat.stream()` yields `StreamEvent::Paused(PauseReason)` and terminates. Resolve the pending tools on `messages` (approve or reject), then call `stream()` again — the core loop picks up where it left off.

```rust
use chat_rs::{Action, ChatBuilder, ScopedCollection, StreamEvent, PauseReason};
use tools_rs::{FunctionCall, ToolCollection, tool};
use serde::Deserialize;

#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
struct ApprovalMeta { requires_approval: bool }

#[tool(requires_approval = true)]
/// Sends an email.
async fn send_email(to: String, subject: String) -> String {
    format!("sent to {to}: {subject}")
}

fn strategy(_call: &FunctionCall, meta: &ApprovalMeta) -> Action {
    if meta.requires_approval { Action::RequireApproval } else { Action::Execute }
}

let tools: ToolCollection<ApprovalMeta> = ToolCollection::collect_tools()?;
let scoped = ScopedCollection::new(tools, strategy);

let mut chat = ChatBuilder::new()
    .with_model(client)
    .with_scoped_tools(scoped)
    .build();

let mut stream = chat.stream(&mut messages).await?;
while let Some(evt) = stream.next().await {
    match evt? {
        StreamEvent::TextChunk(t) => print!("{t}"),
        StreamEvent::Paused(PauseReason::AwaitingApproval { tool_ids }) => {
            for id in tool_ids {
                if let Some(tool) = messages.find_tool_mut(&id) {
                    tool.approve(None); // or tool.reject(Some("denied".into()))
                }
            }
            break;
        }
        _ => {}
    }
}
// Call chat.stream(&mut messages) again to resume the same turn.
```

See `examples/claude/hitl.rs`, `examples/openai/hitl.rs`, and `examples/gemini/hitl.rs` for full interactive REPLs.

## Embeddings

```rust
let client = GeminiBuilder::new()
    .with_model("gemini-embedding-001".to_string())
    .with_embeddings(Some(768))
    .build();

let mut chat = ChatBuilder::new()
    .with_model(client)
    .with_embeddings()
    .build();

let response = chat.embed(&mut messages).await?;
println!("{:?}", response.embeddings);
```

## Native Tools

Provider-specific capabilities beyond standard tool calling:

```rust
// Gemini: Google Search, Code Execution, Google Maps
let client = GeminiBuilder::new()
    .with_model("gemini-2.5-flash".to_string())
    .with_google_search()
    .with_code_execution()
    .build();

// OpenAI: Web Search
let client = OpenAIBuilder::new()
    .with_model("gpt-4o")
    .with_web_search(Some(SearchContextSizeEnum::High), None)
    .build();

// OpenAI: Image Generation
let client = OpenAIBuilder::new()
    .with_model("gpt-4o")
    .with_image_generation(ImageGenerationTool::default())
    .build();
```

## OpenAI-Compatible Endpoints

For any server speaking the OpenAI Chat Completions wire spec (vLLM, llama.cpp's `llama-server`, LiteLLM, etc.), use `ChatCompletionsBuilder` directly:

```rust
use chat_rs::completions::ChatCompletionsBuilder;

let client = ChatCompletionsBuilder::new()
    .with_base_url("http://localhost:8000/v1")
    .with_model("my-model")
    .with_api_key("sk-...")  // optional — omit for servers that don't require auth
    .build();
```

Dedicated wrappers preset URL/env-var/auth for popular targets:

- **Ollama** — `OllamaBuilder` defaults to `http://localhost:11434/v1`, honors `OLLAMA_HOST`, supports `.pull()` to fetch a model via Ollama's native API.
- **Hugging Face Router** — `HuggingFaceBuilder` defaults to `https://router.huggingface.co/v1`, reads `HF_TOKEN`.
- **Cerebras** — `CerebrasBuilder` defaults to `https://api.cerebras.ai/v1`, reads `CEREBRAS_API_KEY`.
- **DeepSeek** — `DeepSeekBuilder` defaults to `https://api.deepseek.com/v1`, reads `DEEPSEEK_API_KEY`.

For endpoints implementing the OpenAI **Responses API** (`POST /responses`, a different wire format from Chat Completions), use `ResponsesBuilder` from the `chat-responses` crate, or `OpenAIBuilder::with_custom_url()` if you want to keep the OpenAI-specific defaults and native tools.

## Router

Route requests across multiple providers with automatic fallback on retryable errors. Add a custom `RoutingStrategy` to control provider selection based on keywords, embeddings, capabilities, or any logic you need.

```rust
use chat_rs::{
    ChatBuilder,
    router::RouterBuilder,
    gemini::GeminiBuilder,
    claude::ClaudeBuilder,
    types::messages,
};

let gemini = GeminiBuilder::new()
    .with_model("gemini-2.5-flash".to_string())
    .build();

let claude = ClaudeBuilder::new()
    .with_model("claude-sonnet-4-20250514".to_string())
    .build();

let router = RouterBuilder::new()
    .add_provider(gemini)
    .add_provider(claude)
    // .with_strategy(my_strategy)  // optional custom routing
    // .circuit_breaker(CircuitBreakerConfig::default())  // optional circuit breaker
    .build();

let mut chat = ChatBuilder::new().with_model(router).build();

let mut msgs = messages::from_user(vec!["Hello!"]);
let res = chat.complete(&mut msgs).await?;
```

Without a custom strategy, the router tries providers in order and falls back on retryable errors (rate limits, network issues). Non-retryable errors are returned immediately.

Enable the optional **circuit breaker** to automatically skip providers that have failed repeatedly, and probe them again after a configurable recovery timeout:

```rust
use chat_rs::router::CircuitBreakerConfig;

let router = RouterBuilder::new()
    .add_provider(gemini)
    .add_provider(claude)
    .circuit_breaker(CircuitBreakerConfig {
        failure_threshold: 3,
        recovery_timeout: std::time::Duration::from_secs(30),
    })
    .build();
```

Streaming is also supported via `StreamRouterBuilder` — enable the `stream` feature flag and use providers that implement `ChatProvider`.

## Transport Layer

Providers are generic over a pluggable `Transport` trait. The default transport is `ReqwestTransport` (HTTP via reqwest) — it's used automatically when you call `.build()` on any builder.

To share an HTTP client across providers:

```rust
use chat_rs::openai::{OpenAIBuilder, ReqwestTransport};

let http = ReqwestTransport::from(my_reqwest_client);
let client = OpenAIBuilder::new()
    .with_model("gpt-4o")
    .with_transport(http.clone()) // Clone shares the connection pool
    .build();
```

To use WebSocket transport (e.g. for OpenAI's Responses API over WS):

```toml
chat-rs = { version = "0.3.0", features = ["openai", "stream", "tokio-tungstenite"] }
```

```rust
use chat_rs::{openai::OpenAIBuilder, transport::AsyncWsTransport};

let ws = AsyncWsTransport::new()
    .with_message_type("response.create"); // OpenAI WS envelope

let client = OpenAIBuilder::new()
    .with_model("gpt-4o")
    .with_transport(ws)
    .build();
```

Two WebSocket transports are available, feature-gated:

| Transport | Feature | Crate | Notes |
|---|---|---|---|
| `AsyncWsTransport` | `tokio-tungstenite` | tokio-tungstenite | Fully async, recommended with tokio |
| `WsTransport` | `tungstenite` | tungstenite | Sync WS bridged via `spawn_blocking` |

To use a fully custom transport (tower, hyper, WASM, etc.):

```rust
use chat_rs::Transport;

struct MyTransport { /* ... */ }
impl Transport for MyTransport { /* ... */ }

let client = OpenAIBuilder::new()
    .with_model("gpt-4o")
    .with_transport(MyTransport::new())
    .build();
```

Transport implementations live in `core/src/transport/impls/`. See [`core/AGENTS.md`](core/AGENTS.md) for the `Transport` trait definition.

## Architecture

```
chat-rs (root)              ← Re-exports + feature flags
├── core/                   ← Traits, types, Chat engine, builder, Transport trait + impls
├── providers/
│   ├── completions/        ← Generic OpenAI Chat Completions wire (`ChatCompletionsBuilder`)
│   ├── responses/          ← Generic OpenAI Responses API wire (`ResponsesBuilder`)
│   ├── gemini/             ← Google Gemini provider
│   ├── claude/             ← Anthropic Claude provider
│   ├── openai/             ← OpenAI (thin wrapper over `chat-responses` + embeddings + native tools)
│   ├── ollama/             ← Ollama wrapper (local daemon, pull/ping)
│   ├── huggingface/        ← Hugging Face Inference Providers (Router)
│   ├── cerebras/           ← Cerebras Inference
│   ├── deepseek/           ← DeepSeek
│   ├── mistralrs/          ← Local in-process inference (mistral.rs)
│   └── router/             ← Multi-provider router
└── examples/
    ├── completions/        ← Generic OAI-compat examples
    ├── gemini/             ← Gemini examples
    ├── claude/             ← Claude examples
    ├── openai/             ← OpenAI examples
    ├── ollama/             ← Ollama examples
    ├── huggingface/        ← Hugging Face examples
    ├── cerebras/           ← Cerebras examples
    ├── deepseek/           ← DeepSeek examples
    ├── mistralrs/          ← mistral.rs (local) examples
    └── router/             ← Router strategy examples
```

See [`core/AGENTS.md`](core/AGENTS.md) and [`providers/AGENTS.md`](providers/AGENTS.md) for detailed architecture documentation.

## Examples

Run examples with the appropriate feature flags:

```bash
# Gemini
cargo run --example gemini-tools --features gemini
cargo run --example gemini-structured --features gemini
cargo run --example gemini-stream --features gemini,stream
cargo run --example gemini-embeddings --features gemini
cargo run --example gemini-code-execution --features gemini
cargo run --example gemini-google-maps --features gemini
cargo run --example gemini-image-understanding --features gemini
cargo run --example gemini-hitl --features gemini,stream
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 cargo run --example gemini-python-tools --features gemini,python

# Claude
cargo run --example claude-completion --features claude
cargo run --example claude-stream --features claude,stream
cargo run --example claude-hitl --features claude,stream

# OpenAI
cargo run --example openai-completion --features openai
cargo run --example openai-stream --features openai,stream
cargo run --example openai-structured --features openai
cargo run --example openai-embeddings --features openai
cargo run --example openai-hitl --features openai,stream
cargo run --example openai-websocket --features openai,stream,tokio-tungstenite

# Router
cargo run --example router-keyword --features router,gemini,claude
cargo run --example router-embeddings --features router,gemini,claude
cargo run --example router-capability --features router,gemini,claude
cargo run --example router-stream --features router,gemini,claude,stream

# Ollama (local)
cargo run --example ollama-completion --features ollama
cargo run --example ollama-stream --features ollama,stream
cargo run --example ollama-tools --features ollama
cargo run --example ollama-structured --features ollama
cargo run --example ollama-embeddings --features ollama
cargo run --example ollama-pull --features ollama

# Hugging Face
cargo run --example huggingface-completion --features huggingface
cargo run --example huggingface-stream --features huggingface,stream

# Cerebras
cargo run --example cerebras-completion --features cerebras
cargo run --example cerebras-stream --features cerebras,stream

# DeepSeek
cargo run --example deepseek-completion --features deepseek
cargo run --example deepseek-stream --features deepseek,stream

# mistral.rs (local in-process)
cargo run --example mistralrs-completion --features mistralrs
cargo run --example mistralrs-stream --features mistralrs,stream
cargo run --example mistralrs-vision --features mistralrs,stream
cargo run --example mistralrs-voice --features mistralrs,stream

# Generic OpenAI-compatible server
cargo run --example completions-completion --features completions

# Retry strategies
cargo run --example retry --features gemini
```

## Minimum Supported Rust Version

Rust **1.94** or later (edition 2024).

## License

MIT
