use chat_rs::{
    chat::ChatBuilder,
    gemini::{self},
    messages::{self, content},
};
use schemars::JsonSchema;
use serde::Deserialize;
use tools_rs::{collect_tools, tool};

/// Produces a short, natural-language sentence describing basic metadata for a user identified by name.
///
/// The returned string is a human-readable sentence that conveys simple preferences and a stylistic note for the named user.
///
/// # Parameters
///
/// - `name`: The user's name to include in the metadata sentence.
///
/// # Returns
///
/// A `String` containing a short natural-language metadata sentence about `name`.
///
/// # Examples
///
/// ```
/// use futures::executor::block_on;
///
/// let meta = block_on(get_user_metadata("Alice".into()));
/// assert!(meta.contains("Alice"));
/// ```
#[tool]
async fn get_user_metadata(name: String) -> String {
    format!("The user {} is a big fan of tacos and burgers. They also like it when you talk like a pirate", name).to_string()
}

#[derive(JsonSchema, Deserialize, Clone, Debug)]
struct User {
    name: String,
    likes: Vec<String>,
}

/// Starts an interactive chat loop that reads lines from stdin, sends them to a configured Gemini chat pipeline, and prints the model's responses.
///
/// The function initializes a Gemini client and chat pipeline, then repeatedly reads a user line, appends it to the conversation, awaits a model completion, and prints the returned response. I/O and chat errors are propagated to the caller.
///
/// # Examples
///
/// ```no_run
/// // Run the compiled binary and interact with it via stdin:
/// // $ cargo run --release --bin your_binary
/// ```
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = gemini::GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        //.with_google_maps(None, false)
        //.with_google_maps(Some((34.050_481, -118.248_526)), false)
        //.with_google_search()
        .build();

    let tools = collect_tools();

    let mut chat = ChatBuilder::new()
        .with_structured_output::<User>()
        //.with_tools(tools)
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

        println!("Found user: {:#?}", response);

        //messages.push(response.clone());

        //println!("Model:\t{:?}", response.parts.last());
    }
}