//! Stream a transcript correction from the Apple on-device model.
//!
//! Run with:
//! ```sh
//! cargo run --example applefm-stream --features applefm,stream
//! ```

use std::io::Write;

use chat_rs::{
    ChatBuilder, StreamEvent,
    applefm::AppleFMBuilder,
    parts,
    types::messages::{self, content},
};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let probe = chat_rs::applefm::availability();
    if !probe.available {
        eprintln!(
            "Apple on-device model unavailable: {}",
            probe.reason.as_deref().unwrap_or("no reason given")
        );
        std::process::exit(1);
    }

    let client = AppleFMBuilder::new().build().map_err(|err| err.err)?;
    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::Messages::default();
    messages.push(content::from_system(parts![
        "You correct raw speech transcripts: fix disfluencies, casing and \
         punctuation without changing meaning. Reply with the corrected text only."
    ]));
    messages.push(content::from_user(parts![
        "so um the meeting is uh moved to thrusday at 3 pee em tell uh tell dario"
    ]));

    let mut stream = chat.stream(&mut messages).await.map_err(|err| err.err)?;
    print!("Corrected: ");
    while let Some(event) = stream.next().await {
        match event? {
            StreamEvent::TextChunk(text) => {
                print!("{text}");
                std::io::stdout().flush()?;
            }
            StreamEvent::Done(response) => {
                println!();
                println!("Metadata: {:?}", response.metadata);
            }
            _ => {}
        }
    }
    Ok(())
}
