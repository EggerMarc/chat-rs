use chat_rs::{
    chat::ChatBuilder,
    gemini::{self},
    messages::{self, content, file::File},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: add from path
    let client = gemini::GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        .build();

    let mut chat = ChatBuilder::new()
        .with_model(client)
        .with_max_steps(5)
        .build();

    let mut messages = messages::Messages::default();

    messages.push(content::from_system(vec![
        "You are a helpful assistant. Your job is to be as useful as possible.",
    ]));

    loop {
        let mut user_input = String::new();
        println!("User:\t");
        std::io::stdin().read_line(&mut user_input)?;
        let mut user_message = content::from_user(vec![&user_input]);
        user_message
            .parts
            .push(messages::parts::PartEnum::File(File::from_url(
                "https://www.youtube.com/watch?v=ZsvZsVPhTVs",
                None,
            )?));

        messages.push(user_message);

        let response = chat.complete(&mut messages).await.map_err(|err| err.err)?;
        println!("Metadata: {:?}", response.metadata);
        println!("Model:\t{:?}", response.content.parts);
    }
}
