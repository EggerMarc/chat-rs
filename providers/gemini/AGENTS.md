# Gemini Provider (`chat-gemini`)

Implements `CompletionProvider`, `StreamProvider`, and `EmbeddingsProvider` for the Google Gemini API.

## API Details

- **Completion endpoint:** `https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent`
- **Streaming endpoint:** `https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse`
- **Embedding endpoint:** `https://generativelanguage.googleapis.com/v1beta/models/{model}:embedContent`
- **Auth:** `x-goog-api-key` header (not Bearer token)
- **Default env var:** `GEMINI_API_KEY`

## Builder

`GeminiBuilder<M, C>` where `M` = `WithoutModel | WithModel`, `C` = `BaseConfig | CompletionConfig | EmbeddingConfig`.

```rust
GeminiBuilder::new()
    .with_model("gemini-2.5-flash".to_string())  // required
    .with_api_key(key)                            // optional, falls back to env
    .with_thoughts(true)                          // enable thinking/reasoning
    .with_code_execution()                        // native tool
    .with_google_search()                         // native tool
    .with_google_search_threshold(0.7)            // native tool with threshold
    .with_google_maps(Some((lat, lng)), true)     // native tool with location
    .with_function_calling_mode("AUTO", None)     // tool config
    .build()
```

For embeddings:
```rust
GeminiBuilder::new()
    .with_model("text-embedding-004".to_string())
    .with_embeddings(Some(768))                   // optional dimensionality
    .with_embeddings_task(EmbeddingsTask::Embed)  // task type
    .build()
```

## Native Tools

| Tool | Builder Method | Description |
|---|---|---|
| `CodeExecutionTool` | `.with_code_execution()` | Lets the model execute code |
| `GoogleSearchTool` | `.with_google_search()` / `.with_google_search_threshold(f32)` | Grounded web search |
| `GoogleMapsTool` | `.with_google_maps(lat_lng, widget)` | Maps integration |

## Request Transformation (`GeminiRequest::from_core`)

- **System messages** are extracted and placed in the `system_instruction` field (not inlined in `contents`)
- **Reasoning parts** are serialized with `thought: true` and include `thought_signature` for round-tripping
- **File (Bytes)** → `inlineData` with base64 encoding + mimetype
- **File (URL)** → `fileData` with `file_uri`
- **FunctionCall** → `functionCall` with `name`, `args`, and `id`
- **FunctionResponse** → `functionResponse` wrapping the response string in an object
- **Structured output** → `response_schema` + `response_mime_type: "application/json"` in generation config
- **Tools** → `functionDeclarations` array + native tool declarations merged in

## Response Parsing

- Extracts first candidate from `candidates` array
- Maps `finishReason`: `STOP` → `Stop`, `MAX_TOKENS` → `MaxTokens`
- Reasoning parts are identified by `thought: true` on text parts
- Token usage from `usageMetadata` → `Metadata.usage`

## Caveats

- **Thoughts require round-tripping:** Gemini returns `thought_signature` on reasoning parts. This signature must be sent back in subsequent requests for multi-turn conversations with thinking enabled. The provider handles this via the `Reasoning` part's signature field.
- **System instruction isolation:** Gemini requires system messages in a separate field. If you mix system and user parts in the same `Content`, the `from_core` transformation splits them.
- **Embeddings are text-only:** The embedding endpoint only accepts `Text` and `Reasoning` parts; `File` and other part types are silently skipped.
- **Native tools and function tools coexist:** Both `functionDeclarations` (user tools) and native tool declarations (e.g., `google_search`) are sent in the same `tools` array, each as a separate object.
- **`function_calling_mode`** accepts `"AUTO"`, `"ANY"`, or `"NONE"`, with an optional `allowed_function_names` list to restrict which functions the model can call.
