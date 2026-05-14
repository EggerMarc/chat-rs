//! Structured output from a local Ollama model.
//!
//! Ollama serves `response_format: json_schema` on its OpenAI-compatible
//! endpoint. Newer models (Llama 3.1+, Qwen2.5) honor the schema; older
//! ones may ignore it and return free text.
//!
//! ```bash
//! ollama pull llama3.2
//! OLLAMA_MODEL=llama3.2 cargo run --example ollama-structured --features ollama
//! ```

use chat_rs::{
    ChatBuilder,
    ollama::OllamaBuilder,
    types::messages::{self, content},
};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(JsonSchema, Deserialize, Clone, Debug)]
struct User {
    pub name: String,
    pub likes: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".to_string());
    let client = OllamaBuilder::new().with_model(model).build();

    let mut chat = ChatBuilder::new()
        .with_structured_output::<User>()
        .with_model(client)
        .build();

    let mut messages = messages::Messages::default();
    messages.push(content::from_system(vec![
        "Extract structured data about the user. Reply ONLY with JSON.",
    ]));

    loop {
        let mut user_input = String::new();
        println!("User:\t");
        std::io::stdin().read_line(&mut user_input)?;
        messages.push(content::from_user(vec![user_input.trim()]));

        let response = chat
            .complete(&mut messages)
            .await
            .map_err(|err| err.err)?
            .expect_complete();
        println!(
            "User {} likes {:?}",
            response.content.name, response.content.likes
        );
    }
}
