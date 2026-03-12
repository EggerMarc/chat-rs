use chat_rs::{ChatBuilder, Messages, gemini, types::messages::content};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = gemini::GeminiBuilder::new()
        .with_model("gemini-embedding-001".to_string())
        .with_embeddings(Some(126))
        .build();

    let mut chat = ChatBuilder::new()
        .with_model(client)
        .with_max_retries(2)
        .build();

    let mut messages = Messages::default();

    let mut flag = 0;
    let parts_to_classify = 10;

    loop {
        flag += 1;
        let mut user_input = String::new();
        println!("User:\t");
        std::io::stdin().read_line(&mut user_input)?;
        let user_message = content::from_user(vec![&user_input]);
        messages.push(user_message);

        if flag % parts_to_classify == 0 {
            let response = chat.embed(&mut messages).await.map_err(|err| err.err)?;
            println!("Model:\t{:?}", response.embeddings);
            flag = 0;
        }
        //println!("Metadata:\t{:?}", response.metadata);
    }
}
