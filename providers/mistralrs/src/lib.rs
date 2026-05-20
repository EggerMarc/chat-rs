//! Local-inference provider for chat-rs, built on [mistral.rs](https://github.com/EricLBuehler/mistral.rs).
//!
//! Loads weights in-process (no HTTP, no daemon). On first use, model files
//! are downloaded into the standard Hugging Face cache (`~/.cache/huggingface/`)
//! using `HF_TOKEN` from the environment when present (only required for
//! gated repos).
//!
//! ```no_run
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! use chat_mistralrs::MistralRsBuilder;
//!
//! let client = MistralRsBuilder::new()
//!     .with_model("Qwen/Qwen2.5-3B-Instruct-GGUF")
//!     .with_gguf_file("qwen2.5-3b-instruct-q4_k_m.gguf")
//!     .build()
//!     .await?;
//! # let _ = client;
//! # Ok(()) }
//! ```
//!
//! See `providers/AGENTS.md` for the overall provider architecture and
//! `ROADMAP-mistralrs.md` for what's in scope per phase.

pub mod api;
pub mod builder;
pub mod client;

pub use builder::{DeviceChoice, MistralRsBuilder, WithModel, WithoutModel};
pub use client::MistralRsClient;

/// Re-exported for ergonomic use in builder calls.
pub use mistralrs::IsqType;
