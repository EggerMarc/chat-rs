//! Chat through the DeepSeek hosted API.
//!
//! ```bash
//! export DEEPSEEK_API_KEY=...
//! cargo run --example deepseek-completion --features deepseek
//! ```

use chat_rs::{ChatBuilder, deepseek::DeepSeekBuilder, parts, types::messages};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();

    let model = std::env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| "deepseek-v4-pro".to_string());

    let client = DeepSeekBuilder::new().with_model(model).build();
    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::from_user(parts!["Say hello in one sentence."]);
    let res = chat.complete(&mut messages).await?.expect_complete();
    println!("Model: {:#?}", res.content);

    Ok(())
}
