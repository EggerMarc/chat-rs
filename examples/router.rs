use chat_rs::{
    ChatBuilder,
    claude::ClaudeBuilder,
    gemini::GeminiBuilder,
    router::RouterBuilder,
    types::messages::{self, content},
};
use tools_rs::{collect_tools, tool};

#[tool]
/// Gets user metadata. Must be called whenever a name is identified.
async fn get_user_metadata(name: String) -> String {
    format!(
        "The user {} is a big fan of tacos and burgers. They also like it when you talk like a pirate",
        name
    )
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // First provider: Claude (tried first)
    let claude = ClaudeBuilder::new()
        .with_model("claude-sonnet-4-20250514".to_string())
        .build();

    // Second provider: Gemini (fallback on rate-limit / network errors)
    let gemini = GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        .build();

    // Router tries Claude first, falls back to Gemini
    let router = RouterBuilder::new()
        .add_provider(claude)
        .add_provider(gemini)
        .build();

    let tools = collect_tools();

    let mut chat = ChatBuilder::new()
        .with_tools(tools)
        .with_model(router)
        .with_max_steps(5)
        .build();

    let mut messages = messages::Messages::default();
    messages.push(content::from_system(vec![
        "You are a helpful assistant. Your job is to be as useful as possible.",
    ]));

    println!("Router: Claude -> Gemini fallback");
    println!("(Type 'My name is [your name]' to test tools)");
    println!("---------------------------------");

    loop {
        let mut user_input = String::new();
        println!("\nUser:");
        std::io::stdin().read_line(&mut user_input)?;
        messages.push(content::from_user(vec![user_input.trim()]));

        let response = chat.complete(&mut messages).await.map_err(|err| err.err)?;
        messages.push(response.content.clone());

        if let Some(text) = response.content.parts.text_response() {
            println!("Model:\t{}", text);
        }
        if let Some(meta) = &response.metadata {
            println!("Provider:\t{}", meta.model_slug.as_deref().unwrap_or("unknown"));
        }
    }
}
