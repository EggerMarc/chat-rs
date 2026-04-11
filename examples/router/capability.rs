use async_trait::async_trait;
use chat_rs::{
    ChatBuilder, Messages, ProviderMeta, claude::ClaudeBuilder, gemini::GeminiBuilder,
    router::RouterBuilder, router::RoutingStrategy, router::StrategyError,
    types::messages::content,
};
use tools_rs::{collect_tools, tool};

#[tool]
/// Gets user metadata. Must be called whenever a name is identified.
async fn get_user_metadata(name: String) -> String {
    format!(
        "The user {} is a big fan of tacos and burgers. They also like it when you talk like a pirate",
        name
    )
}

/// Routes to providers that support custom tools, falling back to any provider.
///
/// Gemini doesn't support native + custom tools simultaneously, so when tools
/// are in play we exclude it and route to Claude instead. Providers opt in
/// by setting `with_metadata("supports_custom_tools", true)` at build time.
struct CapabilityRouter;

#[async_trait]
impl RoutingStrategy for CapabilityRouter {
    async fn rank(
        &self,
        _messages: &Messages,
        providers: &[Option<&ProviderMeta>],
    ) -> Result<Vec<usize>, StrategyError> {
        let capable: Vec<usize> = providers
            .iter()
            .enumerate()
            .filter(|(_, meta)| {
                meta.and_then(|m| m.data.get("supports_custom_tools"))
                    .and_then(|v| v.downcast_ref::<bool>())
                    .copied()
                    .unwrap_or(false)
            })
            .map(|(i, _)| i)
            .collect();

        if capable.is_empty() {
            // No provider declared support — try all in order
            Ok((0..providers.len()).collect())
        } else {
            Ok(capable)
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let claude = ClaudeBuilder::new()
        .with_model("claude-sonnet-4-20250514".to_string())
        .with_metadata("supports_custom_tools", true)
        .build();

    let gemini = GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        .with_metadata("supports_custom_tools", true)
        .build();

    let gemini_search = GeminiBuilder::new()
        .with_model("gemini-2.5-flash".to_string())
        .with_metadata("supports_custom_tools", false)
        .with_google_maps(None, false)
        .with_code_execution()
        .build();

    let router = RouterBuilder::new()
        .add_provider(claude)
        .add_provider(gemini)
        .add_provider(gemini_search)
        .with_strategy(CapabilityRouter)
        .build();

    let tools = collect_tools();

    let mut chat = ChatBuilder::new()
        .with_tools(tools)
        .with_model(router)
        .with_max_steps(5)
        .build();

    let mut messages = Messages::default();
    messages.push(content::from_system(vec![
        "You are a helpful assistant. Your job is to be as useful as possible.",
    ]));

    println!("Capability-routed: only providers with custom tool support");
    println!("(Type 'My name is [your name]' to test tools)");
    println!("-----------------------------------------------------------");

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
