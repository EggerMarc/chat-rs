//! Token-streaming text generation against a local GGUF model loaded via mistral.rs.
//!
//!     cargo run --example mistralrs-stream --features "mistralrs stream chat-mistralrs/metal"

use chat_core::parts;
use chat_core::traits::StreamProvider;
use chat_core::types::messages::from_user;
use chat_core::types::response::StreamEvent;
use chat_mistralrs::MistralRsBuilder;
use futures::StreamExt;
use std::io::{Write, stdout};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = MistralRsBuilder::new()
        .with_model("Qwen/Qwen2.5-0.5B-Instruct-GGUF")
        .with_gguf_file("qwen2.5-0.5b-instruct-q4_k_m.gguf")
        .with_tok_model_id("Qwen/Qwen2.5-0.5B-Instruct")
        .build()
        .await?;

    let mut messages = from_user(parts!["Write a short story about a curious robot."]);
    let mut stream = client.stream(&mut messages, None, None).await?;

    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::TextChunk(s) => {
                print!("{s}");
                stdout().flush()?;
            }
            StreamEvent::Done(_) => {
                println!();
                break;
            }
            _ => {}
        }
    }
    Ok(())
}
