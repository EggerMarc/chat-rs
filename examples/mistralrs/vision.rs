//! Multi-turn vision chat against Qwen2.5-VL loaded locally via mistral.rs.
//!
//! Loads **Qwen/Qwen2.5-VL-3B-Instruct** through mistral.rs's multimodal
//! loader and starts a streaming REPL. At the prompt, type your question;
//! to attach images, **drag image files onto the terminal window** so the
//! path is pasted inline, then press enter. Any token that resolves to an
//! existing image file is stripped from the text and attached as an image
//! part. Multiple images per turn are fine.
//!
//! Example session:
//! ```text
//! you> what is in /Users/me/Downloads/cat.jpg
//! assistant: ...
//! you> compare these two: /tmp/a.png /tmp/b.png
//! assistant: ...
//! ```
//!
//! Terminal quoting/escaping is handled — macOS Terminal pastes `\<space>`,
//! iTerm2 wraps in `'...'`, others just paste the bare path; all three work.
//!
//! ### macOS notes
//! Run with the Metal backend for sane speed. With `IsqType::Q4K` the
//! loaded model lives in roughly 2–3 GB of RAM:
//!
//!     cargo run --release --example mistralrs-vision \
//!       --features "mistralrs stream chat-mistralrs/metal"
//!
//! First run downloads ~6 GB of weights into `~/.cache/huggingface/`. Subsequent
//! runs are warm-cache fast.

use std::io::{self, BufRead, Write};
use std::path::Path;

use chat_core::parts;
use chat_core::types::messages::file::File;
use chat_core::types::messages::parts::PartEnum;
use chat_core::types::messages::{self, content};
use chat_core::types::response::StreamEvent;
use chat_mistralrs::{IsqType, MistralRsBuilder};
use chat_rs::ChatBuilder;
use futures::StreamExt;

const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp", "tif", "tiff"];

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Loading Qwen2.5-VL-3B-Instruct (first run downloads ~6 GB, then stays warm)...");
    let client = MistralRsBuilder::new()
        .with_model("Qwen/Qwen2.5-VL-3B-Instruct")
        .with_multimodal()
        .with_isq(IsqType::Q4K)
        .with_logging()
        .build()
        .await?;
    eprintln!("\nModel ready. Drag images onto the terminal to attach them.\n");

    let mut chat = ChatBuilder::new().with_model(client).build();
    let mut messages = messages::from_system(parts![
        "You are a helpful vision assistant. The user may attach one or \
         more images per turn — answer their question with respect to the \
         attached images when present."
    ]);

    let stdin = io::stdin();
    loop {
        eprint!("you> ");
        io::stderr().flush()?;

        let mut line = String::new();
        if stdin.lock().read_line(&mut line)? == 0 {
            eprintln!();
            break;
        }
        let (text, images) = parse_user_input(&line);
        if text.is_empty() && images.is_empty() {
            continue;
        }

        let prompt = if text.is_empty() {
            "Describe the attached image(s).".to_string()
        } else {
            text
        };
        let mut user_parts: Vec<PartEnum> = vec![prompt.into()];
        for path in &images {
            match File::from_path(path) {
                Ok(f) => user_parts.push(f.into()),
                Err(e) => eprintln!("(skipping {path:?}: {e})"),
            }
        }
        messages.push(content::from_user(user_parts));

        eprint!("assistant: ");
        io::stderr().flush()?;
        let mut stream = chat.stream(&mut messages).await.map_err(|err| err.err)?;
        while let Some(event) = stream.next().await {
            match event.map_err(|err| err.err)? {
                StreamEvent::TextChunk(s) => {
                    print!("{s}");
                    io::stdout().flush()?;
                }
                StreamEvent::Done(_) => {
                    println!("\n");
                    break;
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// Split a line into (text, image_paths). Tokens that resolve to an
/// existing file with an image extension become attachments; everything
/// else is rejoined with single spaces as the prompt text.
fn parse_user_input(line: &str) -> (String, Vec<String>) {
    let tokens = shell_tokenize(line);
    let mut text_tokens = Vec::new();
    let mut images = Vec::new();
    for tok in tokens {
        if is_image_path(&tok) {
            images.push(tok);
        } else {
            text_tokens.push(tok);
        }
    }
    (text_tokens.join(" "), images)
}

fn is_image_path(s: &str) -> bool {
    let p = Path::new(s);
    if !p.is_file() {
        return false;
    }
    p.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| IMAGE_EXTS.iter().any(|x| e.eq_ignore_ascii_case(x)))
}

/// Shell-style tokenize: whitespace separates, single and double quotes
/// span literally, and backslash before a special char (space, parens,
/// quotes, etc.) escapes it — matching how macOS Terminal pastes paths.
fn shell_tokenize(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(c) = chars.next() {
        match c {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '\\' if !in_single => {
                if let Some(&next) = chars.peek() {
                    cur.push(next);
                    chars.next();
                }
            }
            c if c.is_whitespace() && !in_single && !in_double => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::shell_tokenize;

    #[test]
    fn plain_words() {
        assert_eq!(shell_tokenize("what is this"), vec!["what", "is", "this"]);
    }

    #[test]
    fn backslash_escaped_space_macos_terminal() {
        assert_eq!(
            shell_tokenize("describe /tmp/My\\ Folder/cat\\ (1).png please"),
            vec!["describe", "/tmp/My Folder/cat (1).png", "please"]
        );
    }

    #[test]
    fn single_quoted_iterm2() {
        assert_eq!(
            shell_tokenize("describe '/tmp/My Folder/cat (1).png' please"),
            vec!["describe", "/tmp/My Folder/cat (1).png", "please"]
        );
    }

    #[test]
    fn multiple_paths_one_turn() {
        assert_eq!(
            shell_tokenize("compare /tmp/a.png /tmp/b.png"),
            vec!["compare", "/tmp/a.png", "/tmp/b.png"]
        );
    }
}
