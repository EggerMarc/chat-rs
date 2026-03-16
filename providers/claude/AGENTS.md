# Claude Provider (`chat-claude`)

Implements `CompletionProvider` and `StreamProvider` for the Anthropic Claude Messages API. Does not implement `EmbeddingsProvider` — Claude has no embeddings endpoint.

## API Details

- **Completion/Streaming endpoint:** `https://api.anthropic.com/v1/messages`
- **Auth:** `x-api-key` header + `anthropic-version: 2023-06-01`
- **Thinking beta:** `anthropic-beta: interleaved-thinking-2025-05-14` header (sent when thoughts are enabled)
- **Default env var:** `CLAUDE_API_KEY`

## Builder

`ClaudeBuilder<M>` where `M` = `WithoutModel | WithModel`.

```rust
ClaudeBuilder::new()
    .with_model("claude-sonnet-4-20250514".to_string())  // required
    .with_api_key(key)                                    // optional, falls back to env
    .with_thoughts(true)                                  // enable extended thinking (default: true)
    .with_thinking_budget(10000)                          // thinking token budget (default: 10000)
    .with_api_version("2023-06-01".to_string())           // optional, defaults to 2023-06-01
    .build()
```

## Request Transformation (`ClaudeRequest::from_core`)

- **System messages** are extracted and placed in the top-level `system` field (not inlined in `messages`)
- **Reasoning parts** are serialized as `{"type": "thinking", "thinking": "...", "signature": "..."}` for round-tripping
- **File (Bytes)** → `{"type": "image", "source": {"type": "base64", ...}}`
- **File (URL)** → `{"type": "image", "source": {"type": "url", ...}}`
- **FunctionCall** → `{"type": "tool_use", "id": "...", "name": "...", "input": {...}}`
- **FunctionResponse** → `{"type": "tool_result", "tool_use_id": "...", "content": "..."}`; always placed in user-role messages
- **Structured output** → synthetic tool `__structured_output__` with forced `tool_choice`
- **Tools** → `ToolCollection` schemas are converted with `parameters` renamed to `input_schema`
- **Message alternation** — consecutive same-role messages are merged (Claude requires strict user/assistant alternation)
- **Thinking config** — when enabled, adds `{"thinking": {"type": "enabled", "budget_tokens": N}}` and clears `temperature` (Claude requirement)
- **Max tokens** — defaults to 4096 if not provided (Claude requires this field)

## Response Parsing

- Content blocks are tagged by `type`: `text`, `thinking`, `tool_use`
- Maps `stop_reason`: `end_turn` → `Stop`, `max_tokens` → `MaxTokens`, `tool_use` → `ToolCall`
- Thinking blocks capture `signature` for round-tripping via `Reasoning::with_signature()`
- The `__structured_output__` tool use is converted to `PartEnum::Structured` instead of `PartEnum::FunctionCall`
- Token usage from `usage.input_tokens` / `usage.output_tokens` → `Metadata.usage`

## Streaming

Uses `SseParser` from `chat-core` and `Parts::merge_chunk()` for incremental event yielding:

- `content_block_delta` with `text_delta` → `StreamEvent::TextChunk`
- `content_block_delta` with `thinking_delta` → `StreamEvent::ReasoningChunk`
- `content_block_delta` with `signature_delta` → attached to current `Reasoning` part
- `content_block_delta` with `input_json_delta` → accumulated for tool input
- `content_block_stop` (tool_use) → `StreamEvent::ToolCall`
- `message_stop` → `StreamEvent::Done` with full `ChatResponse`

## Tool Use ID Handling

Claude uses string IDs like `"toolu_01A09q..."` for tool use blocks. These are stored directly in `CallId` (which wraps `String` in `tools_core` 0.1.6+). When the core engine produces a `FunctionResponse` with the same `CallId`, the request builder reads it back via `Display` to emit the correct `tool_use_id`.

## Caveats

- **No embeddings** — Claude does not provide an embeddings API. Use a separate provider (Gemini, OpenAI, Voyage AI) for embeddings.
- **Thinking requires signature round-tripping:** Claude returns a `signature` on thinking blocks that must be sent back in subsequent requests. The provider handles this via the `Reasoning` part's `signature` field.
- **Temperature is cleared when thinking is enabled** — Claude requires `temperature` to be unset when extended thinking is active.
- **Tool results must be in user messages** — the request builder automatically assigns `"user"` role to messages containing `tool_result` blocks.
- **Max tokens is required** — unlike some providers, Claude requires `max_tokens` in every request. It defaults to 4096 if not set in `ChatOptions`.
