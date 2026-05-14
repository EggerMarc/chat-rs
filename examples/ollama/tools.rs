//! Tool calling against a local Ollama model.
//!
//! Requires a tool-capable model. Llama 3.1 / 3.2 / Qwen2.5 work well.
//!
//! ```bash
//! ollama pull llama3.2
//! OLLAMA_MODEL=llama3.2 cargo run --example ollama-tools --features ollama
//! ```

use chat_rs::{
    ChatBuilder,
    ollama::OllamaBuilder,
    types::messages::{self, content},
};
use tools_rs::{collect_tools, tool};

#[tool]
/// Gets user metadata. Must be called whenever a name is identified.
async fn get_user_metadata(name: String) -> String {
    format!(
        "The user {name} loves tacos and burgers. They like to be addressed in pirate dialect."
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.2".to_string());
    let client = OllamaBuilder::new().with_model(model).build();

    let tools = collect_tools();

    let mut chat = ChatBuilder::new()
        .with_tools(tools)
        .with_model(client)
        .with_max_steps(5)
        .build();

    let mut messages = messages::Messages::default();
    messages.push(content::from_system(vec![
        "You are a helpful assistant. Call tools whenever they're useful.",
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
        println!("Model:\t{:?}", response.content.parts.last());
        println!("Metadata:\t{:?}", response.metadata);
    }
}
