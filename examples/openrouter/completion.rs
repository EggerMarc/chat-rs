//! Chat through OpenRouter's OpenAI-compatible Responses API.
//!
//! OpenRouter is a gateway in front of many vendors; the model slug is
//! vendor-prefixed (e.g. `anthropic/claude-sonnet-4`, `openai/gpt-4o`).
//!
//! ```bash
//! export OPENROUTER_API_KEY=...
//! cargo run --example openrouter-completion --features openrouter
//! ```

use chat_rs::{ChatBuilder, openrouter::OpenRouterBuilder, parts, types::messages};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();

    let model = std::env::var("OPENROUTER_MODEL")
        .unwrap_or_else(|_| "anthropic/claude-sonnet-4".to_string());

    let client = OpenRouterBuilder::new().with_model(model).build();
    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::from_user(parts!["Say hello in one sentence."]);
    let res = chat.complete(&mut messages).await?.expect_complete();
    println!("Model: {:#?}", res.content);

    Ok(())
}
