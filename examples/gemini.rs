use chat_rs::{chat::ChatBuilder, gemini, messages};
use tools_rs::{collect_tools, tool};

#[tool]
/// Greets the user
async fn greeter(name: String) {
    println!(
        "Hello there, {}! This string contains an easter-egg :)",
        name
    );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = gemini::GeminiClient::new("flash-1.5")?;
    let tools = collect_tools();
    let mut chat = ChatBuilder::new()
        .with_tools(tools)
        .with_model(client)
        .with_max_steps(5)
        .build();

    let mut messages = messages::from_system(vec![
        "You are a helpful assistant. Your job is to be as useful as possible.",
    ]);

    loop {
        let mut user_input = String::new();
        println!("User:\t");
        std::io::stdin().read_line(&mut user_input)?;

        let user_message = messages::from_user(vec![&user_input]);
        messages.extend(user_message);

        let response = chat.complete(&mut messages).await?;
        messages.push(response.clone());
        println!("Model:\t{:?}", response);
    }
}
