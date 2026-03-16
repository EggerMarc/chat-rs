/// NOTE: Claude (Anthropic) does not offer an embeddings API.
///
/// This example demonstrates how you would use the Voyage AI embeddings API
/// (Anthropic's recommended embeddings provider) through a custom
/// EmbeddingsProvider implementation, if one were available.
///
/// For now, this example shows the pattern using the Gemini provider as a
/// reference. If you need embeddings alongside Claude completions, consider
/// using a separate embeddings provider (Gemini, OpenAI, Voyage AI, etc.)
/// alongside your Claude completion client.
///
/// Example with Gemini embeddings + Claude completions:
///
/// ```rust,no_run
/// // Embeddings via Gemini
/// let embed_client = GeminiBuilder::new()
///     .with_model("gemini-embedding-001".to_string())
///     .with_embeddings(Some(256))
///     .build();
///
/// let mut embedder = ChatBuilder::new()
///     .with_model(embed_client)
///     .with_embeddings()
///     .build();
///
/// // Completions via Claude
/// let claude_client = ClaudeBuilder::new()
///     .with_model("claude-sonnet-4-20250514".to_string())
///     .build();
///
/// let mut chat = ChatBuilder::new()
///     .with_model(claude_client)
///     .build();
/// ```
fn main() {
    eprintln!("Claude does not provide an embeddings API.");
    eprintln!("Use a dedicated embeddings provider (Gemini, OpenAI, Voyage AI) instead.");
    eprintln!("See the source of this example for a pattern combining providers.");
    std::process::exit(1);
}
