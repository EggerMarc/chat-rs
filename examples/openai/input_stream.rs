//! Push text into the chat while the model is producing output —
//! `InputStreamed<I>` against OpenAI's Responses API (`chat-openai`,
//! which wraps `chat-responses`).
//!
//! For OpenAI's Responses API (HTTP/SSE, no native bidi WS), each
//! input arrival triggers a re-request with the updated `Messages`:
//! interrupt-and-restart. The engine surface stays the same as it
//! would for a future native-WS provider (planned OpenAI Realtime /
//! Gemini Live), so consumer code is portable across both regimes.
//!
//! ```bash
//! export OPENAI_API_KEY=...
//! cargo run --example openai-input-stream --features openai,stream
//! ```

use std::{io::Write, time::Duration};

use chat_rs::{
    ChatBuilder, PartEnum, StreamEvent,
    openai::OpenAIBuilder,
    types::messages,
};
use futures::{StreamExt, channel::mpsc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());
    let client = OpenAIBuilder::new().with_model(model).build();

    // Unbounded mpsc channel. The receiver implements
    // `Stream<Item = PartEnum> + Send + Unpin + 'static` natively —
    // exactly what `with_input_stream::<I>()` wants.
    let (input_tx, input_rx) = mpsc::unbounded::<PartEnum>();

    let mut chat = ChatBuilder::new()
        .with_model(client)
        .with_input_stream()  // I inferred from chat.stream(_, input_rx) below
        .build();

    let mut messages = messages::from_user(vec![
        "Tell me a long, detailed story about a rust crab who is learning async programming.",
    ]);

    // Spawn an interrupter: after a short delay, push a follow-up into
    // the input stream. The engine will merge it into `Messages` and
    // re-enter the provider with the updated context.
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(2)).await;
        let _ = input_tx.unbounded_send(PartEnum::from(
            "Hold on — make the crab wear a tiny hat too.".to_string(),
        ));
        // Dropping the sender closes the input stream; the engine then
        // drains the provider directly without any further `select`
        // overhead.
        drop(input_tx);
    });

    let mut stream = chat
        .stream(&mut messages, input_rx)
        .await
        .map_err(|f| f.err)?;

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
