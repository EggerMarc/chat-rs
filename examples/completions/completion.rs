//! Interactive chatbot against any OpenAI-compatible server.
//!
//! Point `CompletionsBuilder` at any server that implements the
//! `/v1/chat/completions` wire spec (vLLM, llama.cpp's llama-server,
//! LiteLLM, Cerebras, Groq, Together, Fireworks, Ollama, Mercury, etc.).
//!
//! Set `CHAT_COMPLETIONS_BASE_URL`, `CHAT_COMPLETIONS_MODEL`, and
//! optionally `CHAT_COMPLETIONS_API_KEY` before running:
//!
//! ```bash
//! # Local Ollama:
//! CHAT_COMPLETIONS_BASE_URL=http://localhost:11434/v1 \
//! CHAT_COMPLETIONS_MODEL=llama3 \
//! cargo run --example completions-completion --features completions
//!
//! # Mercury (diffusion LM):
//! CHAT_COMPLETIONS_BASE_URL=https://api.inceptionlabs.ai/v1 \
//! CHAT_COMPLETIONS_MODEL=mercury \
//! CHAT_COMPLETIONS_API_KEY=sk-... \
//! cargo run --example completions-completion --features completions
//! ```
//!
//! Type messages at the prompt; `exit` or Ctrl-D quits.

use std::io::Write;

use chat_rs::{
    ChatBuilder,
    completions::CompletionsBuilder,
    parts,
    types::messages::{self, content},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();

    let base_url = std::env::var("CHAT_COMPLETIONS_BASE_URL")
        .unwrap_or_else(|_| "http://localhost:11434/v1".to_string());
    let model = std::env::var("CHAT_COMPLETIONS_MODEL").unwrap_or_else(|_| "llama3".to_string());

    let mut builder = CompletionsBuilder::new()
        .with_base_url(base_url.clone())
        .with_model(model.clone());

    if let Ok(api_key) = std::env::var("CHAT_COMPLETIONS_API_KEY") {
        builder = builder.with_api_key(api_key);
    }

    let client = builder.build();
    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::Messages::default();

    println!("Chatting with {model} at {base_url} — type 'exit' to quit.");
    println!("--------------------------------------------------------");

    loop {
        let mut user_input = String::new();
        print!("\nUser:\t");
        std::io::stdout().flush()?;
        if std::io::stdin().read_line(&mut user_input)? == 0 {
            break; // EOF (Ctrl-D)
        }
        let user_input = user_input.trim();
        if user_input.is_empty() {
            continue;
        }
        if user_input == "exit" {
            break;
        }

        messages.push(content::from_user(parts![user_input]));

        let response = chat.complete(&mut messages).await?.expect_complete();
        if let Some(text) = response.content.parts.text_response() {
            println!("Model:\t{text}");
        }
        if let Some(metadata) = &response.metadata {
            println!(
                "\t[{} tokens in / {} out, {} ms]",
                metadata.usage.input_tokens,
                metadata.usage.output_tokens,
                metadata
                    .duration_ms
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "?".into()),
            );
        }
    }

    Ok(())
}
