use chat_rs::{
    ChatBuilder,
    StreamEvent, // <-- Import StreamEvent!
    gemini::GeminiBuilder,
    types::messages::{self, content},
};
use futures::StreamExt; // Required to use .next() on the stream
use std::io::Write; // Required to flush stdout instantly
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
    let client = GeminiBuilder::new()
        .with_model("gemini-3.1-pro-preview".to_string())
        .with_thoughts(true)
        .build();

    let tools = collect_tools();

    let mut chat = ChatBuilder::new()
        .with_tools(tools)
        .with_model(client)
        .with_max_steps(5)
        .build();

    let mut messages = messages::Messages::default();
    messages.push(content::from_system(vec![
        "You are a helpful assistant. Your job is to be as useful as possible.",
    ]));

    println!("Start chatting! (Type 'My name is [your name]' to test the tool)");
    println!("---------------------------------------------------------------");

    loop {
        let mut user_input = String::new();
        print!("\nUser:\t");
        std::io::stdout().flush()?;

        std::io::stdin().read_line(&mut user_input)?;
        let user_message = content::from_user(vec![user_input.trim()]);
        messages.push(user_message);

        print!("Model:\t");
        std::io::stdout().flush()?;

        let mut stream = chat.stream(&mut messages).await.map_err(|err| err.err)?;

        // Consume the stream chunk-by-chunk
        while let Some(chunk_res) = stream.next().await {
            match chunk_res {
                Ok(event) => match event {
                    // Normal text prints as standard output
                    StreamEvent::TextChunk(text) => {
                        print!("{}", text);
                    }
                    // Thoughts print in dim gray (\x1b[90m)
                    StreamEvent::ReasoningChunk(reasoning) => {
                        print!("\x1b[90m{}\x1b[0m", reasoning);
                    }
                    // Tools print in yellow (\x1b[33m)
                    StreamEvent::ToolCall(tool_call) => {
                        print!(
                            "\n\x1b[33m[Agent is executing tool: {}]\x1b[0m\n",
                            tool_call.name
                        );
                    }
                    StreamEvent::ToolResult(tool_result) => {
                        print!(
                            "\x1b[33m[Tool {} returned {} bytes]\x1b[0m\nModel:\t",
                            tool_result.name,
                            tool_result.result.to_string().len()
                        );
                    }
                    StreamEvent::Done(_) => {
                        // Stream is fully complete for this turn
                    }
                },
                Err(failure) => {
                    eprintln!("\n[Stream Error]: {:?}", failure);
                    break;
                }
            }
            // Ensure the console updates instantly!
            std::io::stdout().flush()?;
        }

        println!();
    }
}
