use chat_rs::{
    chat::ChatBuilder,
    gemini::{self},
    messages::{self, content},
};
use schemars::JsonSchema;
use tools_rs::{collect_tools, tool};

#[tool]
/// Gets user metadata. Must be called whenever a name is identified.
async fn get_user_metadata(name: String) -> String {
    format!("The user {} is a big fan of tacos and burgers. They also like it when you talk like a pirate", name).to_string()
}

#[derive(JsonSchema)]
struct User {
    name: String,
    likes: Vec<String>,
}

/// Starts an interactive chat loop that sends user input to a configured Gemini model with tools and prints model responses.
///
/// The program builds a Gemini client (configured with a model and optional Google Maps), collects available tools, constructs a chat pipeline with a step limit, then repeatedly reads a line from standard input, sends it to the model, and prints the model's latest response part. Propagates I/O and chat errors via the returned `Result`.
///
/// # Examples
///
/// ```no_run
/// // Run the compiled binary and interact with it via stdin:
/// // $ cargo run --release --bin your_binary
/// ```
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    //let client = gemini::GeminiClient::new("gemini-2.5-flash")?;

    let client = gemini::GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        //.with_google_maps(None, false)
        .with_google_maps(Some((34.050_481, -118.248_526)), false)
        //.with_google_search()
        .build();

    let tools = collect_tools();

    let mut chat = ChatBuilder::new()
        .with_tools(tools)
        .with_model(client)
        .with_max_steps(5)
        //.with_structured_output::<User>()
        .build();

    let mut messages = messages::Messages::default();
    /*
    messages.push(content::from_system(vec![
        "You are a helpful assistant. Your job is to be as useful as possible.",
    ]));
    */

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