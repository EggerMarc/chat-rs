//! Chat through Hugging Face Inference Providers (Router).
//!
//! ```bash
//! export HF_TOKEN=hf_xxx
//! cargo run --example huggingface-completion --features huggingface
//! ```
//!
//! Model slug = `org/model[:provider|:fastest|:cheapest|:preferred]`.
//! Examples: `openai/gpt-oss-120b:fastest`, `meta-llama/Llama-3.3-70B-Instruct:novita`.

use chat_rs::{ChatBuilder, huggingface::HuggingFaceBuilder, parts, types::messages};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();

    let model =
        std::env::var("HF_MODEL").unwrap_or_else(|_| "openai/gpt-oss-120b:fastest".to_string());

    let client = HuggingFaceBuilder::new().with_model(model).build();
    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::from_user(parts!["Say hello in one sentence."]);
    let res = chat.complete(&mut messages).await?.expect_complete();
    println!("Model: {:#?}", res.content);

    Ok(())
}
