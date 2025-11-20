use chat_rs::{chat::ChatBuilder, gemini, messages};
use tools_rs::{collect_tools, tool};

/// Produces a greeting that includes the provided name and a fixed easter-egg message.
///
/// # Examples
///
/// ```no_run
/// #[tokio::main]
/// async fn main() {
///     let msg = greeter("Alice".to_string()).await;
///     assert!(msg.contains("Alice"));
///     assert!(msg.contains("easter-egg"));
/// }
/// ```
#[tool]
async fn greeter(name: String) -> String {
    format!(
        "Hello there, {}! This string contains an easter-egg :)",
        name
    )
}

/// Starts an interactive read–eval–print loop that sends user input to a Gemini language model
/// configured with collected tools and prints the model's responses.
///
/// The function constructs a Gemini client (model "gemini-2.5-flash"), collects available tools,
/// builds a chat session with a `max_steps` limit, seeds the conversation with a system message,
/// and then repeatedly reads lines from standard input, appends them to the conversation, requests
/// a completion from the model, and prints the model's reply.
///
/// # Returns
///
/// `Ok(())` on normal termination. Returns an error if client creation, standard I/O, or model
/// completion fails.
///
/// # Examples
///
/// ```no_run
/// // Build and run the binary, then type input at the prompt to interact with the model.
/// ```
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = gemini::GeminiClient::new("gemini-2.5-flash")?;
    let tools = collect_tools();
    let mut chat = ChatBuilder::new()
        .with_tools(tools)
        .with_model(client)
        .with_max_steps(5)
        .build();

    let mut messages = messages::Messages::default();

    messages.push(messages::from_system(vec![
        "You are a helpful assistant. Your job is to be as useful as possible.",
    ]));

    loop {
        let mut user_input = String::new();
        println!("User:\t");
        std::io::stdin().read_line(&mut user_input)?;
        let user_message = messages::from_user(vec![&user_input]);
        messages.push(user_message);

        let response = chat.complete(&mut messages).await?;
        messages.push(response.clone());
        println!("Model:\t{:?}", response);
    }
}