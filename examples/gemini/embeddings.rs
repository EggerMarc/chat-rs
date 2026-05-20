use chat_rs::{
    ChatBuilder,
    gemini::GeminiBuilder,
    parts,
    types::messages::{self, content},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = GeminiBuilder::new()
        .with_model("gemini-embedding-001".to_string())
        .with_embeddings(Some(126))
        .build();

    let mut chat = ChatBuilder::new()
        .with_model(client)
        .with_embeddings()
        .build();
    let mut messages = messages::Messages::default();

    loop {
        let mut user_input = String::new();
        println!("User:\t");
        std::io::stdin().read_line(&mut user_input)?;
        let user_message = content::from_user(parts![&user_input]);
        messages.push(user_message);

        let response = chat.embed(&mut messages).await.map_err(|err| err.err)?;
        println!("Model:\t{:?}", response.embeddings);
    }
}
