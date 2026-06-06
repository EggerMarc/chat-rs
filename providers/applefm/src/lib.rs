//! Apple on-device foundation model provider for chat-rs.
//!
//! Talks to the ~3B-parameter model that ships with Apple Intelligence
//! (macOS 26+) through Apple's FoundationModels framework. There is no
//! HTTP and no weights file: the OS owns the model; this crate owns the
//! translation.
//!
//! ## How it connects
//!
//! Rust cannot call Swift directly, so the crate embeds a small Swift
//! package (`bridge/`) exposing a few plain C functions. Requests and
//! responses cross that boundary as JSON strings — the same mental model
//! as an HTTP provider's wire format, minus the network. The bridge is
//! compiled automatically by `build.rs`; users only run `cargo build`.
//!
//! On non-macOS targets the crate still compiles (with a stub bridge)
//! and reports the model as unavailable.
//!
//! **Binary crates on macOS need one build.rs line** so the Swift
//! concurrency runtime resolves at load time:
//!
//! ```no_run
//! // build.rs of your binary crate
//! println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
//! ```
//!
//! ## Current scope
//!
//! Text completion (with optional LoRA fine-tune) and availability
//! probing. Tools, structured output, and streaming are rejected at
//! request time for now and arrive in later slices.
//!
//! ```no_run
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! use chat_applefm::AppleFMBuilder;
//!
//! let probe = chat_applefm::availability();
//! if !probe.available {
//!     println!("unavailable: {}", probe.reason.as_deref().unwrap_or("?"));
//!     return Ok(());
//! }
//!
//! let client = AppleFMBuilder::new()
//!     .with_lora("adapters/transcripts.fmadapter") // optional fine-tune
//!     .build()?; // validates config (e.g. the adapter path) upfront
//! // System prompts go through Messages like any other provider; the
//! // provider maps them onto the session's instructions.
//! # Ok(()) }
//! ```
//!
//! ## LoRA fine-tunes
//!
//! [`AppleFMBuilder::with_lora`] loads a `.fmadapter` package — a LoRA
//! trained against the on-device base model with Apple's adapter
//! training toolkit. Adapters are version-locked to the base model:
//! after a macOS update rolls the model, retrain and reship.
//!
//! See `providers/AGENTS.md` for the overall provider architecture.

#![allow(clippy::result_large_err)]

mod api;
mod builder;
mod client;
mod ffi;

pub use builder::AppleFMBuilder;
pub use client::{AppleFMClient, Sampling};

use serde::Deserialize;

/// Result of probing whether the on-device Apple model is usable here.
#[derive(Debug, Clone, Deserialize)]
pub struct Availability {
    /// Whether a session can be created on this machine right now.
    pub available: bool,
    /// Human-readable explanation when `available` is `false`
    /// (ineligible hardware, Apple Intelligence disabled, assets still
    /// downloading, OS too old, or a bridge-less build).
    #[serde(default)]
    pub reason: Option<String>,
}

/// Ask the OS whether the Apple Intelligence on-device model can be used.
///
/// Cheap to call; performs no generation. This is the recommended first
/// call before constructing a client, and what router strategies should
/// consult when deciding whether this provider is eligible at all.
pub fn availability() -> Availability {
    let json = ffi::availability_json();
    serde_json::from_str(&json).unwrap_or_else(|_| Availability {
        available: false,
        reason: Some(format!("malformed bridge reply: {json}")),
    })
}
