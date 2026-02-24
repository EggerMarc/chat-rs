use chat_rs::{
    chat::ChatBuilder,
    gemini,
    messages::{self, content, parts::PartEnum},
};

use once_cell::sync::Lazy;
use regex::Regex;
use url::Url;

static YT_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"(?i)\b((?:https?://)?(?:www\.)?(?:youtube\.com/watch\?v=[\w-]+|youtu\.be/[\w-]+))"#,
    )
    .unwrap()
});

fn find_youtube_urls(text: &str) -> Vec<Url> {
    YT_REGEX
        .captures_iter(text)
        .filter_map(|cap| {
            let mut url = cap.get(1)?.as_str().to_string();
            if !url.starts_with("http") {
                url = format!("https://{}", url);
            }

            Url::parse(&url).ok()
        })
        .collect()
}

/// Runs an interactive loop that reads user input from stdin, attaches a youtube video (from a URL) to each user message,
/// sends the conversation to a Gemini-backed chat model, and prints the model's metadata and content parts.
///
/// The program constructs a Gemini client and a ChatBuilder-backed chat, initializes the conversation with a
/// system prompt, then repeatedly reads a line from stdin, attaches a File part built from a URL to the user message,
/// appends it to the conversation, awaits the model completion, and prints the response.
///
/// # Returns
///
/// `Ok(())` on successful execution; a boxed error (`Box<dyn std::error::Error>`) if any I/O or client error occurs.
///
/// # Examples
///
/// ```no_run
/// // Run the compiled binary and type messages at the prompt. Each message will be sent to the model
/// // with a hardcoded file URL attached; the model's metadata and content parts are printed to stdout.
/// // Example invocation:
/// // $ cargo run --example files
/// ```
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // TODO: add from path
    let client = gemini::GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        .build();

    let mut chat = ChatBuilder::new()
        .with_model(client)
        .with_max_steps(5)
        .build();

    let mut messages = messages::Messages::default();
    let system_messages = content::from_system(vec![
        "You are a helpful assistant. Your job is to be as useful as possible.",
    ]);

    messages.push(system_messages);
    loop {
        let mut user_input = String::new();
        println!("User:\t");
        std::io::stdin().read_line(&mut user_input)?;
        println!("\n");
        let video_parts: Vec<PartEnum> = find_youtube_urls(&user_input)
            .iter()
            .map(|url| {
                PartEnum::from_file(messages::file::File::from_url(url.to_owned(), None).unwrap())
            })
            .collect();

        let mut user_message = content::from_user(vec![&user_input]);
        user_message
            .parts
            .extend(messages::parts::Parts(video_parts));
        messages.push(user_message);

        let response = chat.complete(&mut messages).await.map_err(|err| err.err)?;
        println!("Metadata: {:?}", response.metadata);
        println!("Model:\t{:?}", response.content.parts);
    }
}
