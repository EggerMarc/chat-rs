use chat_rs::{
    ChatBuilder,
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
        .with_api_key("AIzaSyCgSVuLL90e9v264Rfv0-0ImOX58D2oy2Q".to_string())
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

        while let Some(chunk_res) = stream.next().await {
            match chunk_res {
                Ok(text_chunk) => {
                    print!("{}", text_chunk);
                    std::io::stdout().flush()?;
                }
                Err(failure) => {
                    eprintln!("\n[Stream Error]: {:?}", failure);
                    break;
                }
            }
        }

        println!();
    }
}
