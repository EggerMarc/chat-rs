use chat_rs::{
    chat::ChatBuilder,
    gemini,
    messages::{self, content, parts::PartEnum},
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

    messages.push(content::from_system(vec![
        "Describe the provided images clearly and concisely.",
    ]));

    let image_parts: Vec<PartEnum> = image_paths
        .into_iter()
        .map(|path| {
            PartEnum::from_file(
                messages::file::File::from_path(path).expect("Failed to load image file"),
            )
        })
        .collect();

    let mut user_message = content::from_user(vec!["What do you see?"]);
    user_message
        .parts
        .extend(messages::parts::Parts(image_parts));

    messages.push(user_message);

    let response = chat.complete(&mut messages).await.map_err(|e| e.err)?;

    println!("Metadata: {:?}", response.metadata);
    println!("Model:");
    for part in response.content.parts {
        println!("{:?}", part);
    }

    Ok(())
}
