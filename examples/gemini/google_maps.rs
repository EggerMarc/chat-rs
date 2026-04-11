use chat_rs::{
    ChatBuilder,
    gemini::{self},
    types::messages::{self, content},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = gemini::GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        .with_google_maps(Some((34.050_481, -118.248_526)), false)
        .with_google_search()
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
        let user_message = content::from_user(vec![&user_input]);
        messages.push(user_message);

        let response = chat.complete(&mut messages).await.map_err(|err| err.err)?.expect_complete();
        println!("Model:\t{:?}", response.content.parts.last());
        //println!("Metadata:\t{:?}", response.metadata);
    }
}
