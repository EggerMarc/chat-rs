# Roadmap

Tracking upcoming providers and features for chat-rs.

## Provider Status

### Implemented

| Provider | Crate | Completion | SSE Streaming | WebSocket | Embeddings | Native Tools | HITL |
|---|---|---|---|---|---|---|---|
| Google Gemini | `chat-gemini` | Yes | Yes | Planned (Live API) | Yes | Google Search, Code Execution, Google Maps | Yes |
| Anthropic Claude | `chat-claude` | Yes | Yes | — (HTTP-only upstream) | N/A | Extended Thinking | Yes |
| OpenAI | `chat-openai` | Yes | Yes | Yes (Responses API WS) | Yes | Web Search | Yes |

### Planned Providers

| Provider | Priority | Completion | Streaming | Embeddings | Notes |
|---|---|---|---|---|---|
| ~~**Anthropic**~~ | ~~High~~ | ~~Done~~ | ~~Done~~ | ~~N/A~~ | ~~Implemented as `chat-claude`.~~ |
| **Hugging Face** | Medium | Planned | Planned | Planned | Inference API + Inference Endpoints. Wide model catalog. |
| **Cerebras** | Medium | Planned | Planned | Planned | Fast inference. OpenAI-compatible API — may share request layer with `chat-openai`. |
| **AI21** | Medium | Planned | Planned | Planned | Jamba models. |
| **Mistral** | Medium | Planned | Planned | Planned | Mistral/Mixtral models. Has its own API format. |
| **Cohere** | Medium | Planned | Planned | Planned | Command models. Strong embeddings support. |
| **AWS Bedrock** | Low | Planned | Planned | Planned | Multi-model gateway. Requires AWS auth (SigV4), not API key. |
| **Azure OpenAI** | Low | Planned | Planned | Planned | OpenAI models via Azure. Different auth and endpoint format from vanilla OpenAI. |
| **Groq** | Low | Planned | Planned | — | Fast inference. OpenAI-compatible API. |
| **Together AI** | Low | Planned | Planned | Planned | Open model hosting. OpenAI-compatible API. |
| **Ollama** | Low | Planned | Planned | Planned | Local models. Partially works today via `OpenAIBuilder::with_custom_url()` if the endpoint supports Responses API. Dedicated provider would use Ollama's native API. |

### Provider Implementation Notes

**OpenAI-compatible providers** (Cerebras, Groq, Together AI) could potentially share a base with `chat-openai` via custom URLs. However, they may not support the Responses API format. A dedicated provider or a Chat Completions API mode for `chat-openai` may be needed.

**Anthropic** is implemented as `chat-claude`. It uses Claude's Messages API with content blocks and tool use blocks. Extended thinking maps to the `Reasoning` part type with signature round-tripping. No embeddings API — `EmbeddingsProvider` is not implemented.

**AWS Bedrock** and **Azure OpenAI** require non-standard auth (AWS SigV4 / Azure AD tokens) rather than simple API keys. These will need builder extensions for credential configuration.

## Feature Roadmap

### Short Term

- [x] **Anthropic provider** — implemented as `chat-claude`
- [x] **Human in the loop** — pause/resume flows via `ScopedCollection` strategies, `StreamEvent::Paused`, and `Messages::find_tool_mut`
- [x] **Pluggable transport layer** — `Transport` trait in `chat-core` with `send()` and `stream()`, `Request` with scheme/host/path separation. Three built-in implementations (feature-gated): `ReqwestTransport` (HTTP/SSE), `AsyncWsTransport` (tokio-tungstenite), `WsTransport` (tungstenite). BYO transports via trait impl. Providers are generic over `T: Transport`.
- [x] **OpenAI WebSocket streaming** — `AsyncWsTransport` with `.with_message_type("response.create")` connects to `wss://api.openai.com/v1/responses`, authenticates once on handshake, streams events. Connection reuse across calls, terminal event detection, error frame handling.
- [x] **Image generation** — `File` split into kind/source (`#[non_exhaustive]`). OpenAI `image_generation_call` and Gemini `inlineData` image parts decode into `PartEnum::File(File { kind: Image, .. })`. Claude has no image output upstream.

### Medium Term

- [ ] **Hugging Face provider**
- [ ] **Cerebras provider**
- [ ] **AI21 provider**
- [ ] **Mistral provider**
- [ ] **Cohere provider**
- [ ] **Middleware / interceptors** — hook into request/response lifecycle for logging, metrics, or transformation

### Long Term

- [ ] **AWS Bedrock provider**
- [ ] **Azure OpenAI provider**
- [ ] **Ollama native provider**
- [ ] **WASM support** — `transport-wasm` crate using `web-sys` fetch/WebSocket APIs, enabled by the pluggable transport layer
- [ ] **gRPC transport** — `transport-tonic` for providers that support it (Gemini)
- [ ] **Multi-modal output** — audio, video parts (depends on WebSocket transport for low-latency realtime)
- [ ] **Batch API support** — for providers that support batch/async completions
- [ ] **Token counting** — client-side token estimation before sending requests

## Contributing

Want to add a provider? See [`providers/AGENTS.md`](providers/AGENTS.md) for a step-by-step guide on implementing a new provider crate.
