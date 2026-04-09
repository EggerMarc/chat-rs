use async_trait::async_trait;
use chat_rs::{
    ChatBuilder, EmbeddingsProvider, Messages, ProviderMeta,
    claude::ClaudeBuilder,
    gemini::{GeminiBuilder, client::GeminiClient},
    router::RouterBuilder,
    router::RoutingStrategy,
    router::StrategyError,
    types::messages::content,
};

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let (dot, norm_a, norm_b) = a
        .iter()
        .zip(b.iter())
        .fold((0.0_f32, 0.0_f32, 0.0_f32), |(dot, na, nb), (x, y)| {
            (dot + x * y, na + x * x, nb + y * y)
        });
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a.sqrt() * norm_b.sqrt())
}

async fn embed_text(
    embedder: &impl EmbeddingsProvider,
    text: &str,
) -> Result<Vec<f32>, Box<dyn std::error::Error + Send + Sync>> {
    let mut msgs = Messages::default();
    msgs.push(content::from_user(vec![text]));
    let res = embedder.embed(&mut msgs).await.map_err(|e| e.err)?;
    Ok(res.embeddings.content)
}

struct EmbeddingRouter {
    embedder: GeminiClient,
}

#[async_trait]
impl RoutingStrategy for EmbeddingRouter {
    async fn rank(
        &self,
        messages: &Messages,
        providers: &[Option<&ProviderMeta>],
    ) -> Result<Vec<usize>, StrategyError> {
        let query = messages
            .0
            .iter()
            .rev()
            .find_map(|c| c.parts.text_response())
            .map(|t| t.to_string())
            .unwrap_or_default();

        if query.is_empty() {
            return Ok((0..providers.len()).collect());
        }

        let query_emb = embed_text(&self.embedder, &query).await?;

        let mut ranked: Vec<(usize, f32)> = providers
            .iter()
            .enumerate()
            .map(|(i, meta)| {
                let score = meta
                    .and_then(|m| m.data.get("embedding"))
                    .and_then(|v| v.downcast_ref::<Vec<f32>>())
                    .map(|emb| cosine_similarity(&query_emb, emb))
                    .unwrap_or(0.0);
                (i, score)
            })
            .collect();

        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(ranked.into_iter().map(|(idx, _)| idx).collect())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let embedder = GeminiBuilder::new()
        .with_model("gemini-embedding-001".to_string())
        .with_embeddings(Some(256))
        .build();

    let claude_desc = "Best for complex reasoning, planning, and nuanced analysis.";
    let gemini_desc = "Fast model for simple questions, quick answers, and web search.";

    let claude_emb = embed_text(&embedder, claude_desc).await?;
    let gemini_emb = embed_text(&embedder, gemini_desc).await?;

    let claude = ClaudeBuilder::new()
        .with_model("claude-sonnet-4-20250514".to_string())
        .with_description(claude_desc)
        .with_metadata("embedding", claude_emb)
        .build();

    let gemini = GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        .with_description(gemini_desc)
        .with_metadata("embedding", gemini_emb)
        .with_google_search()
        .with_google_maps(None, false)
        .build();

    let strategy = EmbeddingRouter { embedder };

    let router = RouterBuilder::new()
        .add_provider(claude)
        .add_provider(gemini)
        .with_strategy(strategy)
        .build();

    let mut chat = ChatBuilder::new()
        .with_model(router)
        .with_max_steps(5)
        .build();

    let mut messages = Messages::default();
    messages.push(content::from_system(vec![
        "You are a helpful assistant. Your job is to be as useful as possible.",
    ]));

    println!("Embedding-routed: Claude (complex) <-> Gemini (simple)");
    println!("(Type 'My name is [your name]' to test tools)");
    println!("------------------------------------------------------");

    loop {
        let mut user_input = String::new();
        print!("\nUser:\t");
        std::io::Write::flush(&mut std::io::stdout())?;
        std::io::stdin().read_line(&mut user_input)?;
        messages.push(content::from_user(vec![user_input.trim()]));

        let response = chat.complete(&mut messages).await.map_err(|err| err.err)?;

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
