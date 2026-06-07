//! Interactive streaming chat with the Apple on-device foundation model.
//!
//! Run with:
//! ```sh
//! cargo run --example applefm-stream --features applefm,stream
//! # with a LoRA fine-tune:
//! APPLEFM_LORA=path/to/transcripts.fmadapter \
//!     cargo run --example applefm-stream --features applefm,stream
//! ```
//!
//! Type messages at the prompt; `exit` or Ctrl-D quits.

use std::io::Write;

use chat_rs::{
    ChatBuilder, StreamEvent,
    applefm::AppleFMBuilder,
    parts,
    types::messages::{self, content},
};
use futures::StreamExt;

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

    let client = builder.build().map_err(|err| err.err)?;
    // Clones share the session — keep one for prewarm hints while the
    // chat owns the other.
    let prewarmer = client.clone();
    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::Messages::default();
    messages.push(content::from_system(parts![
        "You are a helpful assistant running entirely on this Mac. Keep replies concise."
    ]));

    println!("Streaming from the Apple on-device model — type 'exit' to quit.");
    println!("---------------------------------------------------------------");

    loop {
        // Stage the model while the user types, so the turn that follows
        // the pause doesn't pay warm-up.
        prewarmer.prewarm();

        let mut user_input = String::new();
        print!("\nUser:\t");
        std::io::stdout().flush()?;
        if std::io::stdin().read_line(&mut user_input)? == 0 {
            break; // EOF (Ctrl-D)
        }
        let user_input = user_input.trim();
        if user_input.is_empty() {
            continue;
        }
        if user_input == "exit" {
            break;
        }

        messages.push(content::from_user(parts![user_input]));

        let started = std::time::Instant::now();
        let mut first_token_ms: Option<u128> = None;
        let mut stream = chat.stream(&mut messages).await.map_err(|err| err.err)?;
        print!("Model:\t");
        std::io::stdout().flush()?;
        while let Some(event) = stream.next().await {
            match event? {
                StreamEvent::TextChunk(text) => {
                    first_token_ms.get_or_insert_with(|| started.elapsed().as_millis());
                    print!("{text}");
                    std::io::stdout().flush()?;
                }
                StreamEvent::Done(response) => {
                    println!();
                    if let Some(metadata) = &response.metadata {
                        println!(
                            "\t[{} | first token: {} ms | total: {} ms | prefill: {}]",
                            metadata.model_slug.as_deref().unwrap_or("?"),
                            first_token_ms
                                .map(|d| d.to_string())
                                .unwrap_or_else(|| "?".into()),
                            metadata
                                .duration_ms
                                .map(|d| d.to_string())
                                .unwrap_or_else(|| "?".into()),
                            metadata
                                .provider_specific
                                .get("prefill")
                                .and_then(|v| v.as_str())
                                .unwrap_or("?"),
                        );
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}
