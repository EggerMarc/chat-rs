use chat_rs::{chat::ChatBuilder, gemini, messages, messages::content};

/// Interactive example that reads user lines, sends them to the Gemini embeddings model, and prints the resulting embeddings.
///
/// The program constructs a Gemini embedding client configured for `gemini-embedding-001`, wraps it in a chat client,
/// then repeatedly reads a line from stdin, appends it as a user message, requests embeddings, and prints them.
///
/// # Examples
///
/// ```ignore
/// // Run the example binary and type lines into stdin; each line will produce an embeddings vector printed to stdout.
/// // cargo run --example embeddings
/// ```
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = gemini::GeminiBuilder::new()
        .with_model("gemini-embedding-001".to_string())
        .with_embeddings(Some(126))
        .build();

    let chat = ChatBuilder::new().with_model(client).build();
    let mut messages = messages::Messages::default();

    loop {
        let mut user_input = String::new();
        println!("User:\t");
        std::io::stdin().read_line(&mut user_input)?;
        let user_message = content::from_user(vec![&user_input]);
        messages.push(user_message);

        let response = chat.embed(&mut messages).await.map_err(|err| err.err)?;
        println!("Model:\t{:?}", response.embeddings);
        //println!("Metadata:\t{:?}", response.metadata);
    }
}
