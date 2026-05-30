# chat-core

Core traits, types, and engine for [chat-rs](https://github.com/EggerMarc/chat-rs) — the foundation every provider crate builds on. Exposes the `Chat` engine, `ChatBuilder`, the `CompletionProvider` / `StreamProvider` / `EmbeddingsProvider` traits, the `Transport` trait (with built-in HTTP/SSE and WebSocket impls), and the core message/tool/response types.

You usually don't depend on this crate directly — instead, depend on a provider crate (e.g. [`chat-openai`](https://crates.io/crates/chat-openai), [`chat-gemini`](https://crates.io/crates/chat-gemini), [`chat-completions`](https://crates.io/crates/chat-completions)) which brings `chat-core` in transitively. Bring `chat-core` in explicitly when you want the engine types in your code without pulling the umbrella `chat-rs` crate.

## Install

```toml
[dependencies]
chat-core = "0.3.0"
```

## What's in here

- **`builder::ChatBuilder`** — the type-state builder that wires a provider into a `Chat` engine
- **`chat::Chat`** — the engine; orchestrates the call loop, tool calls, retries, HITL, structured output
- **`traits`** — `CompletionProvider`, `StreamProvider`, `EmbeddingsProvider`, `ChatProvider`
- **`transport`** — `Transport` trait + three built-in impls (feature-gated): `ReqwestTransport` (HTTP/SSE, default), `AsyncWsTransport` (tokio-tungstenite), `WsTransport` (tungstenite)
- **`types`** — `Messages`, `Content`, `Parts`, `Tool`, `ChatOptions`, `ChatResponse`, `StreamEvent`, `Metadata`, etc.
- **`error`** — `ChatError`, `ChatFailure`

## What's new in 0.3

- **`StreamEvent::Structured(Value)`** — providers can yield complete structured objects mid-stream (each event is a whole `serde_json::Value`, not a fragment). The engine accumulates them into `ChatResponse.content.parts` as `PartEnum::Structured` so non-streaming consumers see them too. Drop-in for robotics consumers that produce a stream of typed action steps.
- **`InputStreamed<I>` type-state** — `Chat<CP, InputStreamed<I>>::stream(&mut messages, input: I)` interleaves the model's output stream with a caller-supplied `Stream<Item = PartEnum>` input source. Audio bytes ride as `PartEnum::File`, text as `PartEnum::Text`, tool results as `PartEnum::Tool`. Each input event triggers a case-by-case merge into `Messages` and re-opens the provider stream with the updated state — same interrupt-and-restart pattern HITL already uses, just automated. Builder transition: `ChatBuilder::with_input_stream::<I>()`. Bounds: `I: Stream<Item = PartEnum> + Send + Unpin + 'static`.

Both are additive — existing `Chat<CP, Unstructured>::stream` callers see no behavior change.

## Feature Flags

| Feature | What it enables |
|---|---|
| `reqwest-transport` | HTTP/SSE transport via reqwest (default for most providers) |
| `tokio-tungstenite` | Async WebSocket transport |
| `tungstenite` | Sync WebSocket transport bridged via `spawn_blocking` |
| `stream` | `StreamProvider` trait + streaming machinery |
| `testing` | Test helpers for provider crates |

## Writing a Custom Provider

Implement `CompletionProvider` (and optionally `StreamProvider`, `EmbeddingsProvider`) on your client struct. Use the `Transport` abstraction so users can swap reqwest for their own HTTP/WS client. See [`providers/AGENTS.md`](https://github.com/EggerMarc/chat-rs/blob/main/providers/AGENTS.md) for the conventions every existing provider follows.
