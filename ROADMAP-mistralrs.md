# chat-mistralrs Roadmap

Local inference for chat-rs, built on [mistral.rs](https://github.com/EricLBuehler/mistral.rs). The point of this work is **not** "another Rust LLM crate." The point is to give chat-rs the capabilities it needs to drive agentic, multimodal local workflows — agents that read a screen, ground actions in pixels, and click.

## North star

A chat-rs agent that takes a screenshot via a tool, passes it back to the model as `PartEnum::File`, runs **Qwen2.5-VL** with a structured-output schema constraining the response to `{action: "click"|"type"|"scroll", x, y, text?}`, emits a tool call that performs the action, and loops. If this example works, the PR has done its job.

This is VLA-equivalent functionality assembled from existing chat-rs primitives — no `PartEnum` extensions, no core changes.

## Why mistral.rs (and not vanilla Candle, not llama.cpp bindings)

The engine choice is downstream of the goal. The goal needs five things on day one: image input, **Qwen2.5-VL specifically** (GUI-grounding), grammar-constrained structured outputs, tool calling, and reasonable streaming. Only mistral.rs ships all five in pure Rust today. Vanilla Candle is a per-model porting treadmill; llama.cpp bindings have FFI cost and lag on Qwen2.5-VL; ollama subprocess (already available via `chat-ollama`) lacks Qwen2.5-VL coverage.

mistral.rs is built on Candle, so we keep the "Rust-native, single-binary, no C++" property. It adds the things vanilla Candle doesn't have: per-family vision pipelines, in-flight batching, paged attention, JSON-grammar sampling.

## Relationship to existing providers

`chat-mistralrs` is a **new sibling**, not a replacement for anything.

| Crate | What it does | When to use |
|---|---|---|
| `chat-huggingface` | Calls HF Router (hosted) over HTTPS | Pay-per-token managed inference |
| `chat-ollama` | Talks to a local Ollama daemon over HTTP | Easy local inference, model menu limited to Ollama's catalog |
| `chat-mistralrs` (new) | Loads weights in-process via mistral.rs | Local multimodal/agentic workloads, especially Qwen2.5-VL |

All three can coexist in one client.

## Crate layout

- Path: `providers/mistralrs/`
- Crate name: `chat-mistralrs`
- Workspace member alongside the other providers.
- Cargo feature on the umbrella crate: `mistralrs = ["dep:chat-mistralrs"]`, plus `chat-mistralrs?/stream` in the existing `stream` feature.
- Backend features inside `chat-mistralrs`: `metal`, `cuda`, `accelerate`, `mkl` — off by default, CPU always works.

Structure follows `providers/AGENTS.md`, with the simplification that there is no HTTP transport — the engine lives in-process:

```
providers/mistralrs/src/
├── lib.rs            # MistralRsBuilder (typestate) + public exports
├── client.rs         # Client owning Arc<mistralrs::MistralRs>
└── api/
    ├── mod.rs
    ├── completion.rs # CompletionProvider impl
    ├── stream.rs     # StreamProvider impl (feature-gated)
    └── types/
        ├── request.rs  # Messages + tools + schema -> mistralrs::Request
        ├── response.rs # mistralrs::Response -> ChatResponse / StreamEvent
        └── error.rs
```

## Phases

Each phase is a shippable slice. Phase 1 is the smallest useful PR; the north-star agent example becomes runnable at the end of Phase 4.

---

### Phase 1 — Foundation

- [ ] Crate scaffold + workspace wiring (`Cargo.toml`, umbrella feature, stream feature, example blocks in root `Cargo.toml`).
- [ ] `MistralRsBuilder` typestate matching the other providers' pattern:
  - `WithoutModel` → `WithModel` via `.with_model("org/repo")`.
  - `.with_quant(...)` for GGUF quant selection, optional.
  - `.with_device(Device)` — CPU default, GPU via cargo features.
  - `.with_max_seq_len(usize)`, `.with_paged_attn(bool)` knobs.
  - `.build()` → `MistralRsClient`.
- [ ] HF Hub integration via mistral.rs's built-in loader (which uses `hf-hub` under the hood and respects `~/.cache/huggingface/` + `HF_TOKEN`).
- [ ] `MistralRsClient` owns `Arc<mistralrs::MistralRs>`. Multiple `MistralRsClient` instances can share one engine via builder cloning.
- [ ] `CompletionProvider::complete` impl: map `Messages` → `mistralrs::Request::Normal`, submit via the engine's request sender, await `Response::Done`, map back to `ChatResponse` with a `PartEnum::Text`.
- [ ] `StreamProvider::stream` impl (feature-gated): same submission, but consume the streamed `Response::Chunk` events as `StreamEvent::TextChunk`, emit `Done` at end.
- [ ] `ProviderMeta`: model id, family, quantization, context window, capability flags.
- [ ] First model: pick a small text-only — **Qwen2.5-3B-Instruct** is the right shakedown (same family we'll use for VL later, runs fast on a MacBook).
- [ ] Examples: `examples/mistralrs/completion.rs`, `examples/mistralrs/stream.rs`.
- [ ] Acceptance: text completion and streaming work against Qwen2.5-3B locally, on CPU and (where available) Metal.

**Out of scope for Phase 1:** vision, tools, structured output, family generalization.

---

### Phase 2 — Vision

This is where the PR earns its keep. Get **Qwen2.5-VL** working end-to-end.

- [ ] `MistralRsBuilder::with_vision()` (or auto-detect via mistral.rs's `VisionLoaderBuilder` when the model is a VLM).
- [ ] Map `PartEnum::File` with `image/*` mimetype → `mistralrs::Request::Normal` with attached `DynamicImage` (decode via `image` crate; `File::Url` resolved via `reqwest` or rejected with a clear error in v1 — bytes-only path is simpler).
- [ ] Pass-through of the chat template (mistral.rs owns the per-family template logic).
- [ ] Capability check: rejecting image parts against a non-VLM model returns a clear `ChatFailure`, not a silent drop.
- [ ] Example: `examples/mistralrs/vision.rs` — sends a screenshot file + "describe what you see," gets a description back from Qwen2.5-VL.
- [ ] Acceptance: a PNG screenshot in, a coherent description out, on a MacBook in <30s for a 7B model.

---

### Phase 3 — Structured outputs

This is what turns Qwen2.5-VL into a usable action emitter. Without it, the model's free-form output isn't parseable enough to drive a UI.

- [ ] Honor the `structured_output: Option<&schemars::Schema>` arg on `CompletionProvider::complete`.
- [ ] Map a `schemars::Schema` to mistral.rs's JSON-grammar `ResponseFormat`. mistral.rs already implements grammar-constrained sampling; our job is the adapter.
- [ ] Emit `PartEnum::Structured(serde_json::Value)` instead of `PartEnum::Text` when a schema is supplied and the response validates.
- [ ] Streaming with a schema emits one `StreamEvent::Structured(...)` event at end-of-burst rather than per-token deltas (per-token JSON fragments aren't useful to a consumer).
- [ ] Example: `examples/mistralrs/structured.rs` — define an `Action` struct with `JsonSchema`, get back a deserializable value.
- [ ] Acceptance: feeding Qwen2.5-VL a screenshot + the `Action` schema reliably produces a JSON object that round-trips through `serde_json::from_value::<Action>(...)`.

---

### Phase 4 — Tool calling

mistral.rs has native tool-call support for the families that were trained for it. Our job is to bridge `tools-rs` declarations through.

- [ ] When `tool_declarations: Some(...)` is passed, splice them into mistral.rs's tool format and submit.
- [ ] Streaming parser maps mistral.rs's tool-call events onto `PartEnum::Tool(Tool::new(FunctionCall { ... }))` chunks compatible with `Parts::merge_chunk` (core/src/types/messages/parts.rs:293-365).
- [ ] Tool-result round-trip: resolved `Tool` parts on subsequent `complete()` calls get formatted back into the per-family tool-result wire format that mistral.rs expects.
- [ ] Capability check: requesting tools against a model with no tool-calling adapter in mistral.rs returns a clear `ChatFailure`.
- [ ] Example: `examples/mistralrs/tools.rs` — register a tool via `tools-rs`, run a full call → response → final-answer roundtrip on Qwen2.5-7B-Instruct.
- [ ] **North-star example becomes possible here.** Stitch Phase 2 + 3 + 4 into `examples/mistralrs/screen_agent.rs` (or wherever in `examples/` makes sense): screenshot tool + Qwen2.5-VL + action schema + click tool. If this example works, the PR is done.

---

### Phase 5 — Family expansion (breadth)

Once the agentic core works on Qwen2.5-VL, broaden the menu.

- [ ] Validate against additional VLMs already in mistral.rs: LLaVA-NeXT, MiniCPM-V, Phi-3.5-Vision, Idefics2/3, Mistral-Small-3.1, Pixtral. Each is one example file + integration test, not a per-model port (mistral.rs already did the porting).
- [ ] Validate text-only families: Llama 3.x, Mistral, Phi, Gemma, DeepSeek.
- [ ] Document the supported-models matrix in the crate README.
- [ ] Acceptance: every documented model in the matrix has an example or integration test that runs end-to-end.

---

### Phase 6 — Deferred

Explicitly punted, named so we don't forget the shape:

- **Embeddings.** `EmbeddingsProvider` impl. mistral.rs supports embedding models; wiring is straightforward but not on the critical path for the agentic goal.
- **VLA models proper.** OpenVLA, π0, RT-2 style. Falls out of Phase 3 once you accept "VLM + structured schema ≈ VLA." A true VLA action-token decoder is a follow-up if and when needed.
- **Inference server mode.** Expose the loaded engine over HTTP using mistral.rs's built-in server. Out of scope; users who need this can run `mistralrs-server` directly and point `chat-completions` at it.
- **Speculative decoding, draft models, multi-LoRA.** Power-user features in mistral.rs that aren't needed for the agent loop.

---

## Open questions (decide during Phase 1)

- **One engine per `MistralRsClient`, or shared engine across clients?** Sharing via `Arc<mistralrs::MistralRs>` is cheap and lets one process serve multiple model-aliased clients. Default: one engine per `build()`, but expose a `.with_shared_engine(arc)` escape hatch.
- **Concurrency under `&mut self`.** `CompletionProvider::complete(&mut self, ...)` serializes calls per provider instance even though mistral.rs supports concurrent requests. Users who want concurrent inference clone the client (cheap — it's an `Arc` inside). Document this.
- **Quant selection ergonomics.** GGUF repos contain multiple quants. `.with_quant("Q4_K_M")` filters by suffix; need an escape-hatch `.with_gguf_filename(...)` for cases where that's ambiguous.
- **Image input source.** Phase 2 supports `File::Bytes` only. `File::Url` (remote fetch) is a Phase 5 nice-to-have.
- **Compile-time bloat.** mistral.rs has a large dep tree and slow cold builds. Gate it strictly behind `mistralrs = []` feature so users not opting in don't pay the cost.

## Versioning policy

`chat-mistralrs` **tracks the latest mistral.rs release without pinning**. The intent is to surface upstream API churn early rather than insulate against it — when mistral.rs ships a new capability or breaks an API, we want to know on the next `cargo update`, not months later when we deliberately bump. Breakage from upstream churn is treated as a normal maintenance task, not a regression.

## Contributing

Implementation follows the patterns in [`providers/AGENTS.md`](providers/AGENTS.md), modulo the no-transport simplification. Each phase lands as its own PR; the phase table is the source of truth for what's shipped.
