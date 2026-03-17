# chat-router

A provider router for [chat-rs](https://github.com/EggerMarc/chat-rs) that wraps multiple `CompletionProvider` instances with automatic fallback on transient errors.

## Usage

```rust
use chat_rs::{
    ChatBuilder,
    claude::ClaudeBuilder,
    gemini::GeminiBuilder,
    router::RouterBuilder,
    types::messages::{self, content},
};

let claude = ClaudeBuilder::new()
    .with_model("claude-sonnet-4-20250514".to_string())
    .build();

let gemini = GeminiBuilder::new()
    .with_model("gemini-2.5-flash".to_string())
    .build();

// Tries Claude first, falls back to Gemini on rate-limit / network errors
let router = RouterBuilder::new()
    .add_provider(claude)
    .add_provider(gemini)
    .build();

let mut chat = ChatBuilder::new()
    .with_model(router)
    .with_max_steps(5)
    .build();

let mut messages = messages::Messages::default();
messages.push(content::from_user(vec!["Hello!"]));

let response = chat.complete(&mut messages).await?;
```

## Fallback Behavior

Providers are tried in the order they are added. On a **retryable** error (`RateLimited`, `Network`), the router advances to the next provider. On a **non-retryable** error (`Provider`, `InvalidResponse`, etc.), the error is returned immediately.

## Routing Strategy

The `RoutingStrategy` trait allows custom provider ordering based on the messages. This is currently unimplemented and reserved for a future release.

## Scope

The router implements `CompletionProvider` only. Streaming and embeddings are not supported — use a single provider directly for those.

## Feature Flags

Enable via the root crate:

```toml
chat-rs = { version = "0.0.10", features = ["router", "claude", "gemini"] }
```
