use chat_rs::{ChatBuilder, openai::OpenAIBuilder, parts, types::messages};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();

    let client = OpenAIBuilder::new().with_model("gpt-4o-mini").build();
    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::from_user(parts!["Hey there!"]);

    let res = chat.complete(&mut messages).await?.expect_complete();
    println!("Model: {:#?}", res.content);

    Ok(())
}
