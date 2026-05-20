//! Token-streaming text generation against a local GGUF model loaded via mistral.rs.
//!
//!     cargo run --example mistralrs-stream --features "mistralrs stream chat-mistralrs/metal"

use chat_mistralrs::MistralRsBuilder;
use chat_rs::{ChatBuilder, StreamEvent, parts, types::messages};
use futures::StreamExt;
use std::io::{Write, stdout};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = MistralRsBuilder::new()
        .with_model("Qwen/Qwen2.5-0.5B-Instruct-GGUF")
        .with_gguf_file("qwen2.5-0.5b-instruct-q4_k_m.gguf")
        .with_tok_model_id("Qwen/Qwen2.5-0.5B-Instruct")
        .build()
        .await?;

    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::from_user(parts!["Write a short story about a curious robot."]);
    let mut stream = chat.stream(&mut messages).await.map_err(|err| err.err)?;

    while let Some(event) = stream.next().await {
        match event.map_err(|err| err.err)? {
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
