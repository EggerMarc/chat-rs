use chat_rs::gemini;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    gemini::GeminiClient::new("flash-1.5");
    Ok(())
}
