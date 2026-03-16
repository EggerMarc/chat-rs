use chat_rs::{ChatBuilder, openai::OpenAIBuilder, types::messages};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();

    let client = OpenAIBuilder::new().with_model("gpt-4o-mini").build();
    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::from_user(vec!["Hey there!"]);

    let res = chat.complete(&mut messages).await?;
    println!("Model: {:#?}", res.content);

    Ok(())
}
