use std::io::Write;

use async_trait::async_trait;
use chat_rs::{
    ChatBuilder, Messages, ProviderMeta, StreamEvent,
    claude::ClaudeBuilder,
    gemini::GeminiBuilder,
    parts,
    router::StreamRouterBuilder,
    router::{RoutingStrategy, StrategyError},
    types::messages::content,
};
use futures::StreamExt;
use tools_rs::{collect_tools, tool};

#[tool]
/// Gets user metadata. Must be called whenever a name is identified.
async fn get_user_metadata(name: String) -> String {
    format!(
        "The user {} is a big fan of tacos and burgers. They also like it when you talk like a pirate",
        name
    )
}

/// Routes based on keywords in the last user message.
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
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
            ],
        )
        .build();

    let router = StreamRouterBuilder::new()
        .add_provider(claude)
        .add_provider(gemini)
        .with_strategy(KeywordRouter)
        .build();

    let tools = collect_tools();

    let mut chat = ChatBuilder::new()
        .with_tools(tools)
        .with_model(router)
        .with_max_steps(5)
        .build();

    let mut messages = Messages::default();
    messages.push(content::from_system(parts![
        "You are a helpful assistant. Your job is to be as useful as possible.",
    ]));

    println!("Keyword-routed streaming: Claude (reasoning) <-> Gemini (quick tasks)");
    println!("Try: 'analyze this problem' vs 'quick summary of X'");
    println!("(Type 'My name is [your name]' to test the tool)");
    println!("---------------------------------------------------------------------");

    loop {
        let mut user_input = String::new();
        print!("\nUser:\t");
        std::io::stdout().flush()?;

        std::io::stdin().read_line(&mut user_input)?;
        messages.push(content::from_user(parts![user_input.trim()]));

        let mut stream = chat.stream(&mut messages).await.map_err(|err| err.err)?;

        while let Some(chunk_res) = stream.next().await {
            match chunk_res {
                Ok(event) => match event {
                    StreamEvent::TextChunk(text) => {
                        print!("{}", text);
                    }
                    StreamEvent::ReasoningChunk(reasoning) => {
                        print!("\x1b[90m{}\x1b[0m", reasoning);
                    }
                    StreamEvent::ToolCall(tool_call) => {
                        println!(
                            "\n\x1b[33m[Agent is executing tool: {}]\x1b[0m\n",
                            tool_call.name
                        );
                    }
                    StreamEvent::ToolResult(tool_result) => {
                        println!(
                            "\x1b[33m[Tool {} returned {} bytes]\x1b[0m\n\t",
                            tool_result.name,
                            tool_result.result.to_string().len()
                        );
                    }
                    StreamEvent::Done(response) => {
                        if let Some(meta) = &response.metadata {
                            println!(
                                "\n\x1b[36m[Routed to: {}]\x1b[0m",
                                meta.model_slug.as_deref().unwrap_or("unknown")
                            );
                        }
                    }
                    _ => {}
                },
                Err(failure) => {
                    eprintln!("\n[Stream Error]: {:?}", failure);
                    break;
                }
            }
            std::io::stdout().flush()?;
        }

        println!();
    }
}
