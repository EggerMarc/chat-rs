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

The `RoutingStrategy` trait allows custom provider ordering based on the messages. Implement `RoutingStrategy::rank()` and pass it to `RouterBuilder::with_strategy()` to control provider selection. See the `router-keyword` and `router-capability` examples.

## Circuit Breaker

Enable the optional circuit breaker to automatically skip providers that have failed repeatedly:

```rust
use chat_rs::router::{RouterBuilder, CircuitBreakerConfig};

let router = RouterBuilder::new()
    .add_provider(claude)
    .add_provider(gemini)
    .circuit_breaker(CircuitBreakerConfig {
        failure_threshold: 3,                            // open after 3 consecutive failures
        recovery_timeout: std::time::Duration::from_secs(30), // probe again after 30s
    })
    .build();
```

When a provider's circuit opens, the router skips it until the recovery timeout elapses, then allows a single probe request. A successful probe closes the circuit; a failed probe resets the timer.

## Streaming

Enable the `stream` feature to use `StreamRouterBuilder`, which wraps providers implementing `ChatProvider`:

```toml
chat-rs = { version = "0.1.1", features = ["router", "claude", "gemini", "stream"] }
```

## Scope

The router supports completions and streaming. Embeddings are not supported — use a single provider directly for those.

## Feature Flags

Enable via the root crate:

```toml
chat-rs = { version = "0.1.1", features = ["router", "claude", "gemini"] }
```
