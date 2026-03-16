use chat_rs::{ChatBuilder, claude::ClaudeBuilder, types::messages, types::messages::content};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ClaudeBuilder::new()
        .with_model("claude-sonnet-4-20250514".to_string())
        .build();

    let mut chat = ChatBuilder::new()
        .with_model(client)
        .with_streamed_response()
        .build();

    let mut messages = messages::Messages::default();
    messages.push(content::from_system(vec![
        "You are a helpful assistant.",
    ]));

    loop {
        let mut user_input = String::new();
        println!("\nUser:");
        std::io::stdin().read_line(&mut user_input)?;
        messages.push(content::from_user(vec![&user_input]));

        print!("Model:\t");
        let mut stream = chat.stream(&mut messages).await.map_err(|err| err.err)?;

        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(text) => print!("{}", text),
                Err(err) => {
                    eprintln!("\nStream error: {}", err.err);
                    break;
                }
            }
        }
        println!();
    }
}
