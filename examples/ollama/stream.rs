//! Stream tokens from a local Ollama model.
//!
//! ```bash
//! ollama pull llama3.2
//! OLLAMA_MODEL=llama3.2 cargo run --example ollama-stream --features ollama,stream
//! ```

use chat_rs::{
    ChatBuilder, StreamEvent,
    ollama::OllamaBuilder,
    parts,
    types::messages::{self, content},
};
use futures::StreamExt;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen3.5:4b".to_string());
    let client = OllamaBuilder::new().with_model(model).pull().await?.build();

    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::Messages::default();
    messages.push(content::from_system(parts![
        "You are a helpful assistant. Keep replies short.",
    ]));

    println!("Chat with your local Ollama model. Ctrl-C to quit.");
    println!("---------------------------------------------------");

    loop {
        let mut user_input = String::new();
        print!("\nUser:\t");
        std::io::stdout().flush()?;
        std::io::stdin().read_line(&mut user_input)?;
        messages.push(content::from_user(parts![user_input.trim()]));

        print!("Model:\t");
        std::io::stdout().flush()?;

        let mut stream = chat.stream(&mut messages).await.map_err(|err| err.err)?;
        let mut in_reasoning = false;

        while let Some(chunk_res) = stream.next().await {
            match chunk_res {
                Ok(StreamEvent::ReasoningChunk(thought)) => {
                    print!("\x1b[90m{thought}\x1b[0m");
                    in_reasoning = true;
                    std::io::stdout().flush()?;
                }
                Ok(event) => {
                    if in_reasoning {
                        println!();
                        in_reasoning = false;
                    }
                    match event {
                        StreamEvent::TextChunk(text) => {
                            print!("{text}");
                            std::io::stdout().flush()?;
                        }
                        StreamEvent::Done(res) => {
                            println!("\n[usage] {:?}", res.metadata);
                        }
                        _ => {}
                    }
                }
                Err(failure) => {
                    eprintln!("\n[stream error]: {failure:?}");
                    break;
                }
            }
        }
    }
}
