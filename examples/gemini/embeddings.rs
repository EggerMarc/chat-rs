use chat_rs::{chat::ChatBuilder, gemini, messages, messages::content};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = gemini::GeminiBuilder::new()
        .with_model("gemini-embedding-001:embedContent".to_string())
        .build();

    let chat = ChatBuilder::new().with_model(client).build();
    let mut messages = messages::Messages::default();

    loop {
        let mut user_input = String::new();
        println!("User:\t");
        std::io::stdin().read_line(&mut user_input)?;
        let user_message = content::from_user(vec![&user_input]);
        messages.push(user_message);

        let response = chat.embed(&mut messages).await.map_err(|err| err.err)?;
        println!("Model:\t{:?}", response.embeddings);
        //println!("Metadata:\t{:?}", response.metadata);
    }
}
