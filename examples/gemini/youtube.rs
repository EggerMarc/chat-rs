use chat_rs::{
    ChatBuilder, gemini,
    types::messages::{self, content, parts::PartEnum},
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

/// Extracts YouTube video links from the given text and returns them as parsed `Url` values.
///
/// The function scans the input for YouTube URLs (both `youtube.com/watch?v=...` and `youtu.be/...`),
/// ensures each match starts with a scheme (prepends `https://` when missing), and returns only those
/// that successfully parse as `Url`.
///
/// # Examples
///
/// ```
/// let txt = "Check these: youtube.com/watch?v=abc123 and https://youtu.be/xyz789";
/// let urls = find_youtube_urls(txt);
/// assert_eq!(urls.len(), 2);
/// assert!(urls[0].to_string().contains("youtube.com/watch?v=abc123"));
/// assert!(urls[1].to_string().contains("youtu.be/xyz789"));
/// ```
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

/// Starts an interactive Gemini chat session that reads user input, attaches any found YouTube URLs as video file parts to user messages, sends the conversation to the model, and prints model metadata and content.
///
/// The process runs until terminated; each loop iteration reads a line from stdin, converts discovered YouTube links into message parts, appends them to the conversation, and awaits a model completion which is then printed.
///
/// # Returns
///
/// `Ok(())` on successful exit; an error if reading stdin, parsing URLs/files, or the chat completion fails.
///
/// # Examples
///
/// ```no_run
/// // Build and run the compiled binary, then type messages including YouTube links:
/// // $ cargo run --bin your_binary_name
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

        let response = chat.complete(&mut messages).await.map_err(|err| err.err)?.expect_complete();
        println!("Metadata: {:?}", response.metadata);
        println!("Model:\t{:?}", response.content.parts);
    }
}
