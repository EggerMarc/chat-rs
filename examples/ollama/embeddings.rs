//! Embeddings from a local Ollama embed model.
//!
//! ```bash
//! ollama pull nomic-embed-text
//! OLLAMA_MODEL=nomic-embed-text cargo run --example ollama-embeddings --features ollama
//! ```

use chat_rs::{
    ChatBuilder,
    ollama::OllamaBuilder,
    types::messages::{self, content},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "nomic-embed-text".to_string());
    let client = OllamaBuilder::new().with_model(model).build();

    let mut chat = ChatBuilder::new()
        .with_model(client)
        .with_embeddings()
        .build();
    let mut messages = messages::Messages::default();

    loop {
        let mut user_input = String::new();
        println!("Text:\t");
        std::io::stdin().read_line(&mut user_input)?;
        messages.push(content::from_user(vec![user_input.trim()]));

        let response = chat.embed(&mut messages).await.map_err(|err| err.err)?;
        println!("Dimensions:\t{}", response.embeddings.dimension);
        println!("Embedding[..5]:\t{:?}", &response.embeddings.content[..5]);
        println!("Metadata:\t{:?}", response.metadata);
    }
}
