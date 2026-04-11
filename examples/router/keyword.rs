use async_trait::async_trait;
use chat_rs::{
    ChatBuilder, Messages, ProviderMeta, claude::ClaudeBuilder, gemini::GeminiBuilder,
    router::RouterBuilder, router::RoutingStrategy, router::StrategyError,
    types::messages::content,
};

/// Routes based on keywords in the last user message.
///
/// Each provider stores a list of keywords in its metadata under "keywords".
/// The strategy scores each provider by how many keywords match the user's
/// message and returns them sorted by score (highest first).
struct KeywordRouter;

#[async_trait]
impl RoutingStrategy for KeywordRouter {
    async fn rank(
        &self,
        messages: &Messages,
        providers: &[Option<&ProviderMeta>],
    ) -> Result<Vec<usize>, StrategyError> {
        let query = messages
            .0
            .last()
            .and_then(|c| c.parts.text_response())
            .map(|t| t.0.to_lowercase())
            .unwrap_or_default();

        if query.is_empty() {
            return Ok((0..providers.len()).collect());
        }

        let mut scored: Vec<(usize, usize)> = providers
            .iter()
            .enumerate()
            .map(|(i, meta)| {
                let hits = meta
                    .and_then(|m| m.data.get("keywords"))
                    .and_then(|v| v.downcast_ref::<Vec<String>>())
                    .map(|keywords| {
                        keywords
                            .iter()
                            .filter(|kw| query.contains(kw.as_str()))
                            .count()
                    })
                    .unwrap_or(0);
                (i, hits)
            })
            .collect();

        scored.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(scored.into_iter().map(|(idx, _)| idx).collect())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let claude = ClaudeBuilder::new()
        .with_model("claude-sonnet-4-20250514".to_string())
        .with_metadata(
            "keywords",
            vec![
                "analyze".to_string(),
                "reason".to_string(),
                "explain".to_string(),
                "plan".to_string(),
                "complex".to_string(),
                "compare".to_string(),
            ],
        )
        .build();

    let gemini = GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        .with_metadata(
            "keywords",
            vec![
                "search".to_string(),
                "quick".to_string(),
                "translate".to_string(),
                "summarize".to_string(),
                "simple".to_string(),
                "fast".to_string(),
            ],
        )
        .build();

    let router = RouterBuilder::new()
        .add_provider(claude)
        .add_provider(gemini)
        .with_strategy(KeywordRouter)
        .build();

    let mut chat = ChatBuilder::new()
        .with_model(router)
        .with_max_steps(5)
        .build();

    let mut messages = Messages::default();
    messages.push(content::from_system(vec![
        "You are a helpful assistant. Your job is to be as useful as possible.",
    ]));

    println!("Keyword-routed: Claude (reasoning) <-> Gemini (quick tasks)");
    println!("Try: 'analyze this problem' vs 'quick summary of X'");
    println!("------------------------------------------------------------");

    loop {
        let mut user_input = String::new();
        print!("\nUser:\t");
        std::io::Write::flush(&mut std::io::stdout())?;
        std::io::stdin().read_line(&mut user_input)?;
        messages.push(content::from_user(vec![user_input.trim()]));

        let response = chat.complete(&mut messages).await.map_err(|err| err.err)?.expect_complete();

        if let Some(text) = response.content.parts.text_response() {
            println!("Model:\t{}", text);
        }
        if let Some(meta) = &response.metadata {
            println!(
                "Routed to:\t{}",
                meta.model_slug.as_deref().unwrap_or("unknown")
            );
        }
    }
}
