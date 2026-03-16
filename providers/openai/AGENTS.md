# OpenAI Provider (`chat-openai`)

Implements `CompletionProvider`, `StreamProvider`, and `EmbeddingsProvider` for the OpenAI API.

**This provider uses the Responses API (`/v1/responses`), not the Chat Completions API.**

## API Details

- **Completion endpoint:** `{base_url}/responses`
- **Embedding endpoint:** `{base_url}/embeddings`
- **Default base URL:** `https://api.openai.com/v1`
- **Auth:** `Authorization: Bearer {api_key}` header
- **Default env var:** `OPENAI_API_KEY`

## Builder

`OpenAIBuilder<M, U, C>` where `M` = `WithoutModel | WithModel`, `U` = `BaseEndpoint | CustomEndpoint`, `C` = `BaseConfig | CompletionConfig | EmbeddingConfig`.

```rust
OpenAIBuilder::new()
    .with_model("gpt-4o")                          // required (accepts impl Into<String>)
    .with_api_key(key)                              // optional, falls back to env
    .with_custom_url("http://localhost:8000")        // optional, for local/proxy endpoints
    .with_reasoning_effort("medium")                 // for o1/o3 models
    .with_web_search(Some(SearchContextSizeEnum::High), location)  // native tool
    .without_previous_response_id()                  // disable response_id chaining
    .with_store(true)                                // persist responses server-side
    .build()
```

For embeddings:
```rust
OpenAIBuilder::new()
    .with_model("text-embedding-3-small")
    .with_embeddings()
    .build()
```

## Native Tools

| Tool | Builder Method | Description |
|---|---|---|
| `WebSearchTool` | `.with_web_search(context_size, user_location)` | Web search with configurable context size (Low/Medium/High) and optional user location |

## Response ID Chaining

By default, `use_previous_response_id` is `true`. After each completion, the client stores `last_response_id` and sends it with the next request. This allows the API to maintain conversation context server-side and reduces token usage.

Call `.without_previous_response_id()` on the builder to disable this behavior.

`on_stream_done()` is implemented to capture the response ID after streaming completes.

## Request Transformation (`OpenAIResponsesRequest::from_core`)

- **System messages** are extracted and placed in the `instructions` field
- **User/Model content** goes into the `input` array
- **File (Bytes)** → data URI with base64 (`data:{mimetype};base64,{data}`)
- **File (URL)** → URL string directly
- **FunctionCall / FunctionResponse** — handled via the Responses API's function calling format
- **Structured output** → `text.format` with JSON schema
- **Reasoning effort** → `reasoning` field with `effort` and `summary: "auto"`
- **Tools** → `tools` array combining user tools and native tool declarations
- **`previous_response_id`** → sent if chaining is enabled and a previous ID exists

## Response Parsing

The Responses API returns tagged output items:
- `message` → extract `output_text` → `Text` (or `Structured` if valid JSON with schema)
- `function_call` → `FunctionCall` with `call_id`
- `reasoning` → `Reasoning` with summary text
- `web_search_call` → tracked but not exposed as a part

Metadata includes `response_id`, model slug, and token usage.

## Streaming

Uses SSE with the Responses API event format. Key events:
- `response.output_text.delta` → `TextChunk`
- `response.reasoning_summary_text.delta` → `ReasoningChunk`
- `response.function_call_arguments.delta` → accumulated, emitted as `ToolCall` on completion
- `response.completed` → `Done(ChatResponse)`

A custom `SseParser` handles the event stream parsing.

## Custom Endpoints

`.with_custom_url()` transitions the builder to `CustomEndpoint` state. This enables using OpenAI-compatible APIs (Ollama, vLLM, etc.).

Warnings are emitted when using OpenAI-specific features with custom endpoints:
- `with_reasoning_effort()` — may be rejected
- `with_web_search()` — may be rejected

The API key still falls back to `OPENAI_API_KEY` env var.

## Caveats

- **Responses API only:** This provider does NOT use `/v1/chat/completions`. Custom endpoints must implement the Responses API format (`POST /responses`).
- **Response ID chaining is on by default.** This means the client is stateful — `last_response_id` is mutated after each call. If you clone or share the client, be aware of this.
- **`with_model()` accepts `impl Into<String>`** unlike Gemini which takes `String`. Both work with string literals but Gemini requires `.to_string()`.
- **Embeddings clear all completion config.** Calling `.with_embeddings()` resets native tools, reasoning effort, response ID tracking, and store settings.
- **The `store` option** persists responses on OpenAI's servers. This is off by default (`None`). Set explicitly if needed for fine-tuning data or conversation history.
