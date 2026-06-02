//! Push text into the chat while the model is producing output —
//! `InputStreamed` against OpenAI's Responses API (`chat-openai`, which
//! wraps `chat-responses`).
//!
//! `chat.stream(&mut messages)` returns a `ChatStream`: iterate it with
//! `.next()` for output, push with `.send()`. Here we peel off a producer
//! handle with `.input()` (no `split()` needed — the combined handle stays
//! our reader) and move it into a task that injects a follow-up. Each input
//! arrival merges into `Messages` and re-requests with the updated context:
//! interrupt-and-restart. The surface is identical to what a future native
//! bidi provider (OpenAI Realtime / Gemini Live) would expose, so consumer
//! code is portable across both regimes.
//!
//! ```bash
//! export OPENAI_API_KEY=...
//! cargo run --example openai-input-stream --features openai,stream
//! ```

use std::{io::Write, time::Duration};

use chat_rs::{ChatBuilder, StreamEvent, openai::OpenAIBuilder, types::messages};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());
    let client = OpenAIBuilder::new().with_model(model).build();

    let mut chat = ChatBuilder::new()
        .with_model(client)
        .with_input_stream()
        .build();

    let mut messages = messages::from_user(vec![
        "Tell me a long, detailed story about a rust crab who is learning async programming.",
    ]);

    let mut stream = chat.stream(&mut messages).await.map_err(|f| f.err)?;

    let input = stream.input();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let _ = input.send("Hold on — make the crab wear a tiny hat too.");
    });

    let mut stdout = std::io::stdout().lock();
    while let Some(event) = stream.next().await {
        match event.map_err(|f| f.err)? {
            StreamEvent::TextChunk(text) => {
                write!(stdout, "{}", text)?;
                stdout.flush()?;
            }
            StreamEvent::ReasoningChunk(thought) => {
                write!(stdout, "\x1b[90m{}\x1b[0m", thought)?;
                stdout.flush()?;
            }
            StreamEvent::Done(res) => {
                writeln!(stdout, "\n\n[usage] {:?}", res.metadata)?;
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
