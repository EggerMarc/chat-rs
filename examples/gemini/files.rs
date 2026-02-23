use chat_rs::{
    chat::ChatBuilder,
    gemini::{self},
    messages::{self, content, file::File},
};

/// Runs an interactive loop that reads user input from stdin, attaches a file (from a URL) to each user message,
/// sends the conversation to a Gemini-backed chat model, and prints the model's metadata and content parts.
///
/// The program constructs a Gemini client and a ChatBuilder-backed chat, initializes the conversation with a
/// system prompt, then repeatedly reads a line from stdin, attaches a File part built from a URL to the user message,
/// appends it to the conversation, awaits the model completion, and prints the response.
///
/// # Returns
///
/// `Ok(())` on successful execution; a boxed error (`Box<dyn std::error::Error>`) if any I/O or client error occurs.
///
/// # Examples
///
/// ```no_run
/// // Run the compiled binary and type messages at the prompt. Each message will be sent to the model
/// // with a hardcoded file URL attached; the model's metadata and content parts are printed to stdout.
/// // Example invocation:
/// // $ cargo run --example files
/// ```
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // TODO: add from path
    let client = gemini::GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        .build();

    let mut chat = ChatBuilder::new()
        .with_model(client)
        .with_max_steps(5)
        .build();

    let mut messages = messages::Messages::default();
    let mut system_messages = content::from_system(vec![
        "You are a helpful assistant. Your job is to be as useful as possible.",
    ]);
    system_messages
        .parts
        .push(messages::parts::PartEnum::File(File::from_url(
            "https://www.youtube.com/watch?v=g-ydgmNjReQ",
            None,
        )?));
    messages.push(system_messages);
    loop {
        let mut user_input = String::new();
        println!("User:\t");
        std::io::stdin().read_line(&mut user_input)?;
        let user_message = content::from_user(vec![&user_input]);

        messages.push(user_message);

        let response = chat.complete(&mut messages).await.map_err(|err| err.err)?;
        println!("Metadata: {:?}", response.metadata);
        println!("Model:\t{:?}", response.content.parts);
    }
}

