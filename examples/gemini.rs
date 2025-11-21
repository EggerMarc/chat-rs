use chat_rs::{
    chat::ChatBuilder,
    gemini,
    messages::{self, content},
};
use schemars::JsonSchema;
use tools_rs::{collect_tools, tool};
/// Produces a brief human-readable description for a given user name.
///
/// The returned string is a single sentence that includes the provided name, mentions the user's favorite foods (tacos and burgers), and notes that they enjoy pirate-style speech.
///
/// # Examples
///
/// ```
/// #[tokio::test]
/// async fn example_get_user_metadata() {
///     let s = get_user_metadata("Alice".into()).await;
///     assert!(s.contains("Alice"));
///     assert!(s.contains("tacos"));
///     assert!(s.contains("pirate"));
/// }
/// ```
#[tool]
async fn get_user_metadata(name: String) -> String {
    format!("The user {} is a big fan of tacos and burgers. They also like it when you talk like a pirate", name).to_string()
}

#[derive(JsonSchema)]
struct User {
    name: String,
    likes: Vec<String>,
}

/// Starts an interactive REPL that sends user input to a Gemini model and prints the model's responses.
///
/// The function initializes a Gemini client, collects available tools, builds a chat configured to emit structured output for the `User` type, and then enters a loop that reads lines from standard input, sends them as user messages to the model, appends model responses to the conversation, and prints the last response part to stdout.
///
/// # Examples
///
/// ```no_run
/// // Run the compiled binary and type messages when prompted:
/// // $ cargo run --example gemini
/// ```
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = gemini::GeminiClient::new("gemini-2.5-flash")?;
    let tools = collect_tools();
    let mut chat = ChatBuilder::new()
        .with_tools(tools)
        .with_model(client)
        .with_max_steps(5)
        .with_structured_output::<User>()
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
        println!("Model:\t{:?}", response.parts.last());
    }
}