use chat_rs::{
    ChatBuilder, gemini, parts,
    types::messages::{self, content, file::File},
};

use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Expect:
    // cargo run -- image1.jpg image2.png

    let image_paths: Vec<PathBuf> = env::args().skip(1).map(PathBuf::from).collect();

    if image_paths.is_empty() {
        eprintln!("Provide at least one image path.");
        std::process::exit(1);
    }

    let client = gemini::GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        .build();

    let mut chat = ChatBuilder::new()
        .with_model(client)
        .with_max_steps(3)
        .build();

    let mut messages = messages::Messages::default();

    messages.push(content::from_system(parts![
        "Describe the provided images clearly and concisely.",
    ]));

    let mut user_parts = parts!["What do you see?"];
    user_parts.extend(image_paths.into_iter().map(|path| {
        File::from_path(path)
            .expect("Failed to load image file")
            .into()
    }));
    messages.push(content::from_user(user_parts));

    let response = chat
        .complete(&mut messages)
        .await
        .map_err(|e| e.err)?
        .expect_complete();

    println!("Metadata: {:?}", response.metadata);
    println!("Model:");
    for part in response.content.parts {
        println!("{:?}", part);
    }

    Ok(())
}
