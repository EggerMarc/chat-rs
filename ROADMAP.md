# Roadmap

Tracking upcoming providers and features for chat-rs.

## Provider Status

### Implemented

| Provider | Crate | Completion | SSE Streaming | WebSocket | Embeddings | Native Tools | HITL |
|---|---|---|---|---|---|---|---|
| Google Gemini | `chat-gemini` | Yes | Yes | Planned (Live API) | Yes | Google Search, Code Execution, Google Maps | Yes |
| Anthropic Claude | `chat-claude` | Yes | Yes | — (HTTP-only upstream) | N/A | Extended Thinking | Yes |
| OpenAI | `chat-openai` | Yes | Yes | Yes (Responses API WS) | Yes | Web Search, Image Generation | Yes |
| Generic OAI-compat | `chat-completions` | Yes | Yes | — | Yes | — | Yes |
| Generic Responses API | `chat-responses` | Yes | Yes | Yes (Responses API WS) | — | — | Yes |
| Ollama | `chat-ollama` | Yes | Yes | — | Yes | — | Yes |
| Hugging Face Router | `chat-huggingface` | Yes | Yes | — | — (not on OAI-compat surface) | — | Yes |
| Cerebras | `chat-cerebras` | Yes | Yes | — | — (not on OAI-compat surface) | — | Yes |
| DeepSeek | `chat-deepseek` | Yes | Yes | — | — (not on OAI-compat surface) | — | Yes |
| mistral.rs (local) | `chat-mistralrs` | Yes (text, image, audio) | Yes | — (planned) | Planned | Planned (tools, structured outputs) | Yes |

`chat-completions` is the shared Chat Completions wire client. `chat-responses` is the symmetric Responses API wire client; `chat-openai` is now a thin wrapper over it (presets URL + `OPENAI_API_KEY`, adds OpenAI-specific native tools `web_search`/`image_generation`, and the OpenAI-specific `/embeddings` endpoint). `chat-ollama`, `chat-huggingface`, `chat-cerebras`, and `chat-deepseek` are thin wrappers over `chat-completions` that preset URLs, auth, and provider-specific niceties (e.g. Ollama's `pull()` against the native API). DeepSeek's open-weight V4 models natively emit DSML-tagged (XML-ish) tool calls; the hosted API normalizes them to standard OpenAI JSON before returning, so `chat-deepseek` sees only JSON. A future local-DeepSeek runtime would need its own DSML parser in the local-inference layer (e.g. `chat-mistralrs`), not in `chat-deepseek`.

`chat-mistralrs` is different in kind: it loads weights in-process via [mistral.rs](https://github.com/EricLBuehler/mistral.rs) — no HTTP, no daemon. Geared at local multimodal/agentic workflows (Qwen2.5-VL, structured outputs for actions, tool calling). Text, image, and audio inputs work today; structured outputs, tool calling, and broader family coverage are still on the way. Tracks the latest mistral.rs release without pinning; upstream churn is treated as normal maintenance.

### Planned Providers

| Provider | Priority | Completion | Streaming | Embeddings | Notes |
|---|---|---|---|---|---|
| ~~**Anthropic**~~ | ~~High~~ | ~~Done~~ | ~~Done~~ | ~~N/A~~ | ~~Implemented as `chat-claude`.~~ |
| ~~**Hugging Face**~~ | ~~Medium~~ | ~~Done~~ | ~~Done~~ | ~~—~~ | ~~Implemented as `chat-huggingface` (Inference Providers / Router).~~ |
| ~~**Cerebras**~~ | ~~Medium~~ | ~~Done~~ | ~~Done~~ | ~~—~~ | ~~Implemented as `chat-cerebras`.~~ |
| ~~**Ollama**~~ | ~~Low~~ | ~~Done~~ | ~~Done~~ | ~~Done~~ | ~~Implemented as `chat-ollama` (Ollama's OpenAI-compatible endpoint + native `/api/pull`).~~ |
| **Groq** | Medium | Planned | Planned | — | Supports both Chat Completions and Responses API. Trivial via `chat-completions` once a `chat-groq` wrapper lands; Responses path waits on `chat-responses`. |
| **vLLM / llama.cpp / LiteLLM** | Medium | Works today | Works today | Works today | Use `ChatCompletionsBuilder::with_base_url(...)` directly. Dedicated wrappers optional. |
| **AI21** | Medium | Planned | Planned | Planned | Jamba models. |
| **Mistral** | Medium | Planned | Planned | Planned | Mistral/Mixtral models. Has its own API format. |
| **Cohere** | Medium | Planned | Planned | Planned | Command models. Strong embeddings support. |
| **AWS Bedrock** | Low | Planned | Planned | Planned | Multi-model gateway. Requires AWS auth (SigV4), not API key. |
| **Azure OpenAI** | Low | Planned | Planned | Planned | OpenAI models via Azure. Different auth and endpoint format from vanilla OpenAI. |
| **Together AI** | Low | Planned | Planned | Planned | Open model hosting. OpenAI-compatible API — would wrap `chat-completions`. |

### Provider Implementation Notes

**OpenAI-compatible providers** that speak the Chat Completions wire (Cerebras, DeepSeek, Groq, Together AI, vLLM, llama.cpp, Ollama, HF Router) share the `chat-completions` crate as their wire-spec layer. New wrappers just preset URL + auth + provider-specific extras.

**A planned `chat-responses` crate** will factor the OpenAI Responses API wire out of `chat-openai` the same way `chat-completions` factors Chat Completions. Providers that support both (Groq) will then be able to toggle between wire specs on the builder.

**Anthropic** is implemented as `chat-claude`. It uses Claude's Messages API with content blocks and tool use blocks. Extended thinking maps to the `Reasoning` part type with signature round-tripping. No embeddings API — `EmbeddingsProvider` is not implemented.

**AWS Bedrock** and **Azure OpenAI** require non-standard auth (AWS SigV4 / Azure AD tokens) rather than simple API keys. These will need builder extensions for credential configuration.

## Feature Roadmap

### Short Term

- [x] **Anthropic provider** — implemented as `chat-claude`
- [x] **Human in the loop** — pause/resume flows via `ScopedCollection` strategies, `StreamEvent::Paused`, and `Messages::find_tool_mut`
- [x] **Pluggable transport layer** — `Transport` trait in `chat-core` with `send()` and `stream()`, `Request` with scheme/host/path separation. Three built-in implementations (feature-gated): `ReqwestTransport` (HTTP/SSE), `AsyncWsTransport` (tokio-tungstenite), `WsTransport` (tungstenite). BYO transports via trait impl. Providers are generic over `T: Transport`.
- [x] **OpenAI WebSocket streaming** — `AsyncWsTransport` with `.with_message_type("response.create")` connects to `wss://api.openai.com/v1/responses`, authenticates once on handshake, streams events. Connection reuse across calls, terminal event detection, error frame handling.
- [x] **Image generation** — `File` split into kind/source (`#[non_exhaustive]`). OpenAI `image_generation_call` and Gemini `inlineData` image parts decode into `PartEnum::File(File { kind: Image, .. })`. Claude has no image output upstream.
- [x] **Mid-stream structured events** — `StreamEvent::Structured(Value)` variant for providers that emit complete typed objects mid-stream (robotics action steps, etc.). Engine accumulates into `ChatResponse.content.parts` as `PartEnum::Structured` for non-streaming-equivalent semantics. (chat-core 0.3.0)
- [x] **Input-stream type-state** — `Chat<CP, InputStreamed>::stream(&mut messages)` returns a `ChatStream`: the output stream you iterate with `.next()`, carrying an input side you push to with `.send()` (the inverse of `.next()`), with `split()` into independent `(InputStream, OutputStream)` halves and `cancel()` to tear down. Pushed input rides as `PartEnum` (audio = `File`, mapped caller-side before `send`), coalesces into the trailing user turn, and restarts the provider stream — same interrupt-and-restart pattern as HITL, now push-driven. The producer handle is `Clone + Send + 'static`, so it drops into a task; completed tool work survives interrupts (only the in-flight partial is discarded). Native-WS providers (planned Realtime/Live) keep their session open in client state; trait contract stays unchanged. (redesigned in chat-core 0.4.0)

### Medium Term

- [x] **Generic Chat Completions wire crate** — shipped as `chat-completions`. Foundation for all OAI-compat providers.
- [x] **Hugging Face provider** — shipped as `chat-huggingface` (Inference Providers / Router).
- [x] **Cerebras provider** — shipped as `chat-cerebras`.
- [x] **DeepSeek provider** — shipped as `chat-deepseek` (hosted API only; local-weight DSML parsing belongs in the local-runtime layer).
- [x] **Ollama provider** — shipped as `chat-ollama` with native `/api/pull` support.
- [x] **Generic Responses API wire crate** — shipped as `chat-responses`. Factored out of `chat-openai`; unblocks Groq Responses path and shared WS groundwork. Native tools are passed in as pre-materialized `Value`s, so the wire crate is trait-agnostic.
- [ ] **Groq provider** — both Chat Completions and Responses paths, the latter waits on `chat-responses`.
- [ ] **AI21 provider**
- [ ] **Mistral provider**
- [ ] **Cohere provider**
- [ ] **Middleware / interceptors** — hook into request/response lifecycle for logging, metrics, or transformation

### Long Term

- [ ] **AWS Bedrock provider**
- [ ] **Azure OpenAI provider**
- [ ] **WASM support** — `transport-wasm` crate using `web-sys` fetch/WebSocket APIs, enabled by the pluggable transport layer
- [ ] **gRPC transport** — `transport-tonic` for providers that support it (Gemini)
- [ ] **Multi-modal output** — audio, video parts (depends on WebSocket transport for low-latency realtime)
- [ ] **Batch API support** — for providers that support batch/async completions
- [ ] **Token counting** — client-side token estimation before sending requests

## Contributing

Want to add a provider? See [`providers/AGENTS.md`](providers/AGENTS.md) for a step-by-step guide on implementing a new provider crate.
