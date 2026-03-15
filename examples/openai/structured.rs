use chat_rs::{
    ChatBuilder,
    openai::OpenAIBuilder,
    types::messages::{self, content},
};
use schemars::JsonSchema;
use serde::Deserialize;
use tools_rs::{collect_tools, tool};

#[tool]
/// Gets user information. Must be called whenever a name is identified.
async fn get_user_metadata(name: String) -> String {
    format!(
        "The user {} is a big fan of tacos and burgers. The user {} hates spinach and broccoli",
        name, name
    )
}

#[derive(JsonSchema, Deserialize, Clone, Debug)]
struct User {
    pub name: String,
    pub likes: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenAIBuilder::new()
        .with_model("gpt-4o-mini")
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
        println!(
            "User {} likes {:?}",
            response.content.name, response.content.likes
        );
    }
}
