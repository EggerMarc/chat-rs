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
//! ## Current scope
//!
//! This is the first slice: probing model availability.
//!
//! ```no_run
//! let probe = chat_applefm::availability();
//! if probe.available {
//!     println!("on-device model ready");
//! } else {
//!     println!("unavailable: {}", probe.reason.as_deref().unwrap_or("?"));
//! }
//! ```

mod ffi;

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
