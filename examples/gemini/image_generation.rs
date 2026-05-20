use chat_rs::{
    ChatBuilder, PartEnum, gemini, parts,
    types::messages::{self, file::FileSource},
};
use std::fs;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    dotenvy::dotenv().ok();

    // Image output requires a Gemini model that supports it.
    let client = gemini::GeminiBuilder::new()
        .with_model("gemini-2.5-flash-image-preview".to_string())
        .with_image_output()
        .build();

    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::from_user(parts![
        "Generate an image of a small wooden cabin in a snowy forest at dusk.",
    ]);

    let res = chat.complete(&mut messages).await?.expect_complete();

    let mut saved = 0;
    for part in res.content.parts {
        if let PartEnum::File(file) = part
            && file.is_image()
            && let FileSource::Bytes(bytes) = file.source
        {
            let ext = file.mime.as_str().rsplit('/').next().unwrap_or("png");
            let path = format!("gemini_image_{saved}.{ext}");
            fs::write(&path, &bytes)?;
            println!("Wrote {} ({} bytes)", path, bytes.len());
            saved += 1;
        }
    }

    if saved == 0 {
        eprintln!("No image returned. Model output: {:#?}", res.metadata);
    }

    Ok(())
}
