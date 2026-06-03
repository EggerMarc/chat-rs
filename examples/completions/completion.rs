//! Bring-your-own OpenAI-compatible server.
//!
//! Point `CompletionsBuilder` at any server that implements the
//! `/v1/chat/completions` wire spec (vLLM, llama.cpp's llama-server,
//! LiteLLM, Cerebras, Groq, Together, Fireworks, Ollama, etc.).
//!
//! Set `CHAT_COMPLETIONS_BASE_URL`, `CHAT_COMPLETIONS_MODEL`, and
//! optionally `CHAT_COMPLETIONS_API_KEY` before running:
//!
//! ```bash
//! CHAT_COMPLETIONS_BASE_URL=http://localhost:11434/v1 \
//! CHAT_COMPLETIONS_MODEL=llama3 \
//! cargo run --example completions-completion --features completions
//! ```

use chat_rs::{ChatBuilder, completions::CompletionsBuilder, parts, types::messages};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();

    let base_url = std::env::var("CHAT_COMPLETIONS_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
    let model = std::env::var("CHAT_COMPLETIONS_MODEL").unwrap_or_else(|_| "llama3".to_string());

    let mut builder = CompletionsBuilder::new()
        .with_base_url(base_url)
        .with_model(model);

    if let Ok(api_key) = std::env::var("CHAT_COMPLETIONS_API_KEY") {
        builder = builder.with_api_key(api_key);
    }

    let client = builder.build();
    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::from_user(parts!["Say hello in one sentence."]);
    let res = chat.complete(&mut messages).await?.expect_complete();
    println!("Model: {:#?}", res.content);

    Ok(())
}
