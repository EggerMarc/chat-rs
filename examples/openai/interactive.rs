//! Interactive CLI chat with mid-response barge-in — `InputStreamed` against
//! OpenAI's Responses API.
//!
//! Type a message and watch the reply stream back. Keep typing *while* it's
//! still streaming: each line you enter is pushed into the live stream with
//! `input.send(..)`, the engine merges it and re-enters the provider with the
//! updated context (interrupt-and-restart). When a reply finishes, your next
//! line starts a fresh turn.
//!
//! This is the realtime-text-input shape from the design: one persistent
//! stdin reader feeds lines, `split()` gives us an independent producer
//! (`input`) and reader (`output`), and a `select!` races model output
//! against your keystrokes. Type `/quit` to exit.
//!
//! ```bash
//! export OPENAI_API_KEY=...
//! cargo run --example openai-interactive --features openai,stream
//! ```

use std::io::Write;

use chat_rs::{
    ChatBuilder, StreamEvent,
    openai::OpenAIBuilder,
    types::messages::{Messages, content},
};
use futures::StreamExt;
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());
    let client = OpenAIBuilder::new().with_model(model.clone()).build();

    let mut chat = ChatBuilder::new()
        .with_model(client)
        .with_input_stream()
        .with_max_steps(8)
        .build();

    // One long-lived stdin reader, feeding lines into a channel. Owning stdin
    // in a single task avoids fighting over it across turns; the main loop
    // pulls a line as a turn's prompt, and pulls further lines mid-turn as
    // barge-in.
    let (lines_tx, mut lines_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        let mut reader = BufReader::new(tokio::io::stdin()).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            if lines_tx.send(line).is_err() {
                break;
            }
        }
    });

    println!("Chatting with {model}. Type while it replies to barge in. /quit to exit.");

    let mut messages = Messages::default();

    'session: loop {
        print!("\n\x1b[1;32m> \x1b[0m");
        std::io::stdout().flush()?;

        // The turn's opening prompt.
        let prompt = match lines_rx.recv().await {
            Some(line) => line,
            None => break, // stdin closed (EOF)
        };
        if prompt.trim() == "/quit" {
            break;
        }
        messages.push(content::from_user([prompt]));

        let (input, mut output) = chat.stream(&mut messages).await.map_err(|f| f.err)?.split();

        let mut stdout = std::io::stdout().lock();
        loop {
            tokio::select! {
                // Model output.
                event = output.next() => {
                    match event {
                        Some(Ok(StreamEvent::TextChunk(text))) => {
                            write!(stdout, "{text}")?;
                            stdout.flush()?;
                        }
                        Some(Ok(StreamEvent::ReasoningChunk(thought))) => {
                            write!(stdout, "\x1b[90m{thought}\x1b[0m")?;
                            stdout.flush()?;
                        }
                        Some(Ok(StreamEvent::Done(_))) | None => {
                            writeln!(stdout)?;
                            break; // turn over → next prompt
                        }
                        Some(Ok(_)) => {}
                        Some(Err(failure)) => {
                            writeln!(stdout, "\n[error] {}", failure.err)?;
                            break;
                        }
                    }
                }
                // A line typed mid-reply → barge in. `send` only fails if the
                // turn already finished, in which case it's the next prompt.
                line = lines_rx.recv() => {
                    match line {
                        Some(l) if l.trim() == "/quit" => break 'session,
                        Some(l) => {
                            writeln!(stdout, "\n\x1b[33m[+ {}]\x1b[0m", l.trim())?;
                            let _ = input.send(l);
                        }
                        None => break 'session,
                    }
                }
            }
        }
    }

    println!("bye");
    Ok(())
}
