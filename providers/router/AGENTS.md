# Router (`chat-router`)

Implements `CompletionProvider` by wrapping multiple providers with fallback semantics. Does not implement `StreamProvider` or `EmbeddingsProvider`.

## Architecture

```
providers/router/src/
├── lib.rs          # RouterBuilder (typestate: WithoutProvider → WithProvider)
├── router.rs       # Router struct + CompletionProvider impl
└── strategy.rs     # RoutingStrategy trait + StrategyError type
```

## Builder

`RouterBuilder<M>` where `M` = `WithoutProvider | WithProvider`. The first `add_provider()` call transitions to `WithProvider`, which unlocks `build()`.

```rust
RouterBuilder::new()
    .add_provider(claude_client)   // transitions to WithProvider
    .add_provider(gemini_client)   // additional providers
    .build()                       // → Router
```

`with_strategy(strategy)` accepts any `impl RoutingStrategy + 'static` to control provider ordering.

## Router Struct

```rust
pub struct Router {
    providers: Vec<Box<dyn CompletionProvider>>,
    strategy: Option<Box<dyn RoutingStrategy>>,
}
```

Providers are stored as trait objects (`Box<dyn CompletionProvider>`) to support heterogeneous provider types (e.g., mixing `ClaudeClient` and `GeminiClient`).

## CompletionProvider Implementation

1. Determine provider ordering: use `strategy.rank()` if set, otherwise insertion order `[0, 1, ..., n-1]`
2. For each provider in order:
   - Call `provider.complete(messages, tools, options, structured_output)`
   - On success → return response
   - On retryable error (`ChatError::is_retryable()` = true) → try next provider
   - On non-retryable error → return error immediately
3. If all providers exhausted → return the last retryable error

## Error Retryability

The router uses `ChatError::is_retryable()` (added in chat-core 0.0.4):

| Error | Retryable | Rationale |
|---|---|---|
| `RateLimited` | Yes | Transient, another provider may have capacity |
| `Network` | Yes | Transient connection failure |
| `Provider` | No | Request-level issue (400, 404, 500, etc.) |
| `InvalidResponse` | No | Malformed response, switching provider won't help the same request |
| `MaxStepsExceeded` | No | Client-side limit |
| `Callback` | No | User callback error |
| `Other` | No | Unknown, fail-safe |

## RoutingStrategy Trait

```rust
pub type StrategyError = Box<dyn std::error::Error + Send + Sync>;

#[async_trait]
pub trait RoutingStrategy: Send + Sync {
    async fn rank(
        &self,
        messages: &Messages,
        providers: &[Box<dyn CompletionProvider>],
    ) -> Result<Vec<usize>, StrategyError>;
}
```

Allows custom provider ordering based on message content. The `rank` method is async (to support strategies that need async work like embedding lookups), receives the full `providers` slice for inspection, and returns a `Result` — returning `StrategyError` on failure so the router can handle strategy errors gracefully. See the `examples/router/` directory for keyword, embedding, and capability-based strategies.

## Caveats

- **Completions only** — streaming and embeddings are not routed. Use a single provider for those.
- **No state sharing** — each provider in the router is independent. There is no shared conversation state across providers.
- **Message truncation** — the Chat engine's retry loop truncates messages before each retry. The router does not modify messages between provider attempts.
- **Metadata** — the response metadata (model slug, usage) reflects whichever provider succeeded. Failed provider attempts do not contribute metadata.
