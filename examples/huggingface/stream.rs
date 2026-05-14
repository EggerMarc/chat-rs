//! Stream from HF Inference Providers.
//!
//! ```bash
//! export HF_TOKEN=hf_xxx
//! HF_MODEL=deepseek-ai/DeepSeek-R1:fastest \
//!   cargo run --example huggingface-stream --features huggingface,stream
//! ```

use chat_rs::{
    ChatBuilder, StreamEvent,
    huggingface::HuggingFaceBuilder,
    types::messages::{self, content},
};
use futures::StreamExt;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let model =
        std::env::var("HF_MODEL").unwrap_or_else(|_| "openai/gpt-oss-120b:fastest".to_string());

    let client = HuggingFaceBuilder::new().with_model(model).build();
    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::Messages::default();
    messages.push(content::from_system(vec![
        "You are a helpful assistant. Keep replies short.",
    ]));

    loop {
        let mut user_input = String::new();
        print!("\nUser:\t");
        std::io::stdout().flush()?;
        std::io::stdin().read_line(&mut user_input)?;
        messages.push(content::from_user(vec![user_input.trim()]));

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
