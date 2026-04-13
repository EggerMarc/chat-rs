use chat_rs::{
    ChatBuilder, StreamEvent,
    openai::OpenAIBuilder,
    transport::AsyncWsTransport,
    types::messages::{self, content},
};
use futures::StreamExt;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    // Use the async WebSocket transport instead of the default HTTP/SSE.
    // Authenticates once on the WS handshake; subsequent requests reuse
    // the connection with no per-request auth overhead.
    let ws = AsyncWsTransport::new()
        .with_message_type("response.create");

    let client = OpenAIBuilder::new()
        .with_model("gpt-4o-mini")
        .with_transport(ws)
        .build();

    let mut chat = ChatBuilder::new()
        .with_model(client)
        .with_max_steps(5)
        .build();

    let mut messages = messages::Messages::default();
    messages.push(content::from_system(vec!["You are a helpful assistant."]));

    println!("OpenAI over WebSocket (type 'quit' to exit)");
    println!("--------------------------------------------");

    loop {
        let mut user_input = String::new();
        print!("\nUser:\t");
        std::io::stdout().flush()?;
        std::io::stdin().read_line(&mut user_input)?;

        let input = user_input.trim();
        if input.eq_ignore_ascii_case("quit") {
            break;
        }

        messages.push(content::from_user(vec![input]));

        let mut stream = chat.stream(&mut messages).await.map_err(|err| err.err)?;

        print!("Model:\t");
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(StreamEvent::TextChunk(text)) => print!("{text}"),
                Ok(StreamEvent::Done(res)) => {
                    if let Some(meta) = &res.metadata {
                        println!(
                            "\n\t[tokens: in={} out={}]",
                            meta.usage.input_tokens, meta.usage.output_tokens
                        );
                    }
                }
                Ok(e) => {
                    println!("{}", e)
                }
                Err(e) => {
                    eprintln!("\n[Error]: {e:?}");
                    break;
                }
            }
            std::io::stdout().flush()?;
        }
    }

    Ok(())
}
