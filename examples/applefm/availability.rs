//! Probe whether this machine can run the Apple on-device foundation model.
//!
//! Run with:
//! ```sh
//! cargo run --example applefm-availability --features applefm
//! ```
//!
//! Exits 0 when the model is usable, 1 otherwise (with the reason).

fn main() {
    let probe = chat_rs::applefm::availability();
    if probe.available {
        println!("Apple on-device model: AVAILABLE");
    } else {
        println!(
            "Apple on-device model: UNAVAILABLE — {}",
            probe.reason.as_deref().unwrap_or("no reason given")
        );
        std::process::exit(1);
    }
}
