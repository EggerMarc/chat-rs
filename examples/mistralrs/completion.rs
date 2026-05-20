//! Minimal text completion against a local GGUF model loaded via mistral.rs.
//!
//! First run downloads the model into `~/.cache/huggingface/` (a few GB).
//! Subsequent runs are warm-cache fast.
//!
//! Run with the `metal` feature on macOS for GPU acceleration:
//!     cargo run --example mistralrs-completion --features "mistralrs chat-mistralrs/metal"

use chat_mistralrs::MistralRsBuilder;
use chat_rs::{ChatBuilder, parts, types::messages};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = MistralRsBuilder::new()
        .with_model("Qwen/Qwen2.5-0.5B-Instruct-GGUF")
        .with_gguf_file("qwen2.5-0.5b-instruct-q4_k_m.gguf")
        .with_tok_model_id("Qwen/Qwen2.5-0.5B-Instruct")
        .build()
        .await?;

    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::from_user(parts!["Write a haiku about the ocean."]);
    let response = chat.complete(&mut messages).await?.expect_complete();

    if let Some(text) = response.content.parts.text_response() {
        println!("{}", text.as_str());
    }
    Ok(())
}
