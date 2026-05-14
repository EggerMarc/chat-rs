//! Pull a model from the Ollama registry, then chat with it.
//!
//! `.pull()` hits Ollama's native `/api/pull` endpoint and returns the
//! builder, so it slots into the normal chain before `.build()`. If the
//! model is already present locally the call returns near-instantly;
//! otherwise it blocks until the download finishes.
//!
//! ```bash
//! OLLAMA_MODEL=qwen2.5:0.5b cargo run --example ollama-pull --features ollama
//! ```

use chat_rs::{
    ChatBuilder,
    ollama::OllamaBuilder,
    types::messages,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();

    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5:0.5b".to_string());

    eprintln!("Pulling {model} (this may take a while on first run)...");
    let client = OllamaBuilder::new()
        .with_model(model)
        .pull()
        .await?
        .build();
    eprintln!("Pull complete.");

    let mut chat = ChatBuilder::new().with_model(client).build();
    let mut messages = messages::from_user(vec!["In one sentence, what are you?"]);
    let res = chat.complete(&mut messages).await?.expect_complete();
    println!("Model: {:#?}", res.content);

    Ok(())
}
