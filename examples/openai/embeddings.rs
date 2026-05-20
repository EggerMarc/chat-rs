use chat_rs::{
    ChatBuilder,
    openai::OpenAIBuilder,
    parts,
    types::messages::{self, content},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let client = OpenAIBuilder::new()
        .with_model("text-embedding-3-small")
        .with_embeddings()
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
        println!("Dimensions:\t{}", response.embeddings.dimension);
        println!("Embedding:\t{:?}", &response.embeddings.content[..5]);
        println!("Metadata:\t{:?}", response.metadata);
    }
}
