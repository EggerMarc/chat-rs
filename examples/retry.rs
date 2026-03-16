use chat_rs::{
    ChatBuilder, gemini,
    macros::retry_strategy,
    types::messages::{self, content},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = gemini::GeminiBuilder::new()
        .with_model("gibberish-model".to_string())
        .build();

    let retry_model = ChatBuilder::new().with_model(client);

    let mut chat = ChatBuilder::new()
        .with_model(client)
        .with_max_retries(5)
        .with_retry_strategy(retry_strategy!(|ctx| {
            println!("Retrying for the {}th time", ctx.idx);
            tokio::time::sleep(tokio::time::Duration::from_millis((400 * ctx.idx).into())).await;
        }))
        .build();

    let mut messages = messages::Messages::default();

    messages.push(content::from_system(vec!["Talk to me like an 8 year old."]));

    let user_message = content::from_user(vec!["Hello?"]);

    messages.push(user_message);

    return match chat.complete(&mut messages).await {
        Ok(_) => unreachable!(),
        Err(err) => {
            println!("Metadata: {:?}", err.metadata);
            println!("Error: {:?}", err.err.to_string());
            Ok(())
        }
    };
}
