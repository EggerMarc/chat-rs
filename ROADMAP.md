# Roadmap

Tracking upcoming providers and features for chat-rs.

## Provider Status

### Implemented

| Provider | Crate | Completion | Streaming | Embeddings | Native Tools |
|---|---|---|---|---|---|
| Google Gemini | `chat-gemini` | Yes | Yes | Yes | Google Search, Code Execution, Google Maps |
| OpenAI | `chat-openai` | Yes | Yes | Yes | Web Search |

### Planned Providers

| Provider | Priority | Completion | Streaming | Embeddings | Notes |
|---|---|---|---|---|---|
| **Anthropic** | High | Planned | Planned | N/A | Claude models. Messages API. No native embeddings endpoint. |
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

**Anthropic** has a unique message format with content blocks and tool use blocks. It also supports extended thinking, which maps to the existing `Reasoning` part type. No embeddings API exists — the `EmbeddingsProvider` trait would not be implemented.

**AWS Bedrock** and **Azure OpenAI** require non-standard auth (AWS SigV4 / Azure AD tokens) rather than simple API keys. These will need builder extensions for credential configuration.

## Feature Roadmap

### Short Term

- [ ] **Anthropic provider** — highest priority missing provider
- [ ] **Image generation** — support image output parts from models that can generate images

### Medium Term

- [ ] **Hugging Face provider**
- [ ] **Cerebras provider**
- [ ] **AI21 provider**
- [ ] **Mistral provider**
- [ ] **Cohere provider**
- [ ] **Configurable HTTP client** — allow users to pass a pre-configured `reqwest::Client` (for proxies, custom TLS, timeouts)
- [ ] **Middleware / interceptors** — hook into request/response lifecycle for logging, metrics, or transformation

### Long Term

- [ ] **AWS Bedrock provider**
- [ ] **Azure OpenAI provider**
- [ ] **Ollama native provider**
- [ ] **Multi-modal output** — audio, video parts
- [ ] **Batch API support** — for providers that support batch/async completions
- [ ] **Token counting** — client-side token estimation before sending requests

## Contributing

Want to add a provider? See [`providers/AGENTS.md`](providers/AGENTS.md) for a step-by-step guide on implementing a new provider crate.
