//! Chat through Cerebras Inference.
//!
//! ```bash
//! export CEREBRAS_API_KEY=...
//! cargo run --example cerebras-completion --features cerebras
//! ```

use chat_rs::{ChatBuilder, cerebras::CerebrasBuilder, types::messages};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();

    let model =
        std::env::var("CEREBRAS_MODEL").unwrap_or_else(|_| "llama-3.3-70b".to_string());

    let client = CerebrasBuilder::new().with_model(model).build();
    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::from_user(vec!["Say hello in one sentence."]);
    let res = chat.complete(&mut messages).await?.expect_complete();
    println!("Model: {:#?}", res.content);

    Ok(())
}
