//! Hit a local (or remote) Ollama daemon.
//!
//! Start Ollama with `ollama serve` and pull a model:
//! ```bash
//! ollama pull llama3
//! cargo run --example ollama-completion --features ollama
//! ```
//!
//! Override host with `OLLAMA_HOST=http://10.0.0.5:11434` if the daemon
//! isn't local.

use chat_rs::{ChatBuilder, ollama::OllamaBuilder, parts, types::messages};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();

    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3".to_string());

    let client = OllamaBuilder::new().with_model(model).build();
    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::from_user(parts!["Say hello in one sentence."]);
    let res = chat.complete(&mut messages).await?.expect_complete();
    println!("Model: {:#?}", res.content);

    Ok(())
}
