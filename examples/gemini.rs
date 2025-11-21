use chat_rs::{
    chat::ChatBuilder,
    gemini,
    messages::{self, content},
};
use tools_rs::{collect_tools, tool};

#[tool]
/// Gets user metadata
async fn get_user_metadata(name: String) -> String {
    format!("The user {} is a big fan of tacos and burgers. They also like it when you talk like a pirate", name).to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = gemini::GeminiClient::new("gemini-2.0-flash")?;
    let tools = collect_tools();
    let mut chat = ChatBuilder::new()
        .with_tools(tools)
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

        let response = chat.complete(&mut messages).await?;
        messages.push(response.clone());
        println!("Model:\t{:?}", response);
    }
}
