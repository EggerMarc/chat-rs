//! One transcript correction through the Apple on-device model.
//!
//! Run with:
//! ```sh
//! cargo run --example applefm-completion --features applefm
//! # with a LoRA fine-tune:
//! APPLEFM_LORA=path/to/transcripts.fmadapter \
//!     cargo run --example applefm-completion --features applefm
//! ```

use chat_rs::{ChatBuilder, applefm::AppleFMBuilder, parts, types::messages, types::messages::content};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let probe = chat_rs::applefm::availability();
    if !probe.available {
        eprintln!(
            "Apple on-device model unavailable: {}",
            probe.reason.as_deref().unwrap_or("no reason given")
        );
        std::process::exit(1);
    }

    let mut builder = AppleFMBuilder::new();
    if let Ok(lora) = std::env::var("APPLEFM_LORA") {
        println!("Using LoRA adapter: {lora}");
        builder = builder.with_lora(lora);
    }

    let mut chat = ChatBuilder::new().with_model(builder.build()).build();

    let mut messages = messages::Messages::default();
    messages.push(content::from_system(parts![
        "You correct raw speech transcripts: fix disfluencies, casing and \
         punctuation without changing meaning. Reply with the corrected text only."
    ]));
    messages.push(content::from_user(parts![
        "so um the meeting is uh moved to thrusday at 3 pee em tell uh tell dario"
    ]));

    let response = chat
        .complete(&mut messages)
        .await
        .map_err(|err| err.err)?
        .expect_complete();

    if let Some(text) = response.content.parts.text_response() {
        println!("Corrected: {text}");
    }
    println!("Metadata: {:?}", response.metadata);
    Ok(())
}
