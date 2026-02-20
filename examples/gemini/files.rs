use chat_rs::{
    chat::ChatBuilder,
    gemini::{self},
    messages::{self, content},
};
use schemars::JsonSchema;
use serde::Deserialize;
use tools_rs::{collect_tools, tool};

#[derive(JsonSchema, Deserialize, Clone, Debug)]
struct User {
    name: String,
    likes: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: add from path
    let client = gemini::GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        .build();

    let tools = collect_tools();

    let mut chat = ChatBuilder::new()
        .with_structured_output::<User>()
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

        let response = chat.complete(&mut messages).await.map_err(|err| err.err)?;
        println!("Found user: {:#?}", response);
    }
}
