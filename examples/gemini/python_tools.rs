//! Load tools from Python scripts via the tools-rs `python` adapter, then
//! hand them to a Gemini chat. The scripts live in `python_scripts/`; each
//! file is scanned for `@tool`-decorated functions and registered with the
//! `ToolCollection` exactly like a native `#[tool]` would be.

use chat_rs::{
    ChatBuilder,
    gemini,
    types::messages::{self, content},
};
use tools_rs::{Language, ToolsBuilder};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = gemini::GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        .build();

    let tools = ToolsBuilder::new()
        .with_language(Language::Python)
        .from_path("examples/gemini/python_scripts")
        .collect()?;

    let mut chat = ChatBuilder::new()
        .with_tools(tools)
        .with_model(client)
        .with_max_steps(5)
        .build();

    let mut messages = messages::Messages::default();
    messages.push(content::from_system(vec![
        "You are a helpful assistant. Use the available tools when relevant.",
    ]));

    loop {
        let mut user_input = String::new();
        println!("User:\t");
        std::io::stdin().read_line(&mut user_input)?;
        let user_message = content::from_user(vec![&user_input]);
        messages.push(user_message);

        let response = chat
            .complete(&mut messages)
            .await
            .map_err(|err| err.err)?
            .expect_complete();
        println!("Model:\t{:?}", response.content.parts.last());
        println!("Metadata:\t{:?}", response.metadata);
    }
}
