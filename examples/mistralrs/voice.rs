//! Voice chat against a local audio-multimodal model loaded via mistral.rs.
//!
//! Records from the system microphone, sends the audio to **Gemma 3n E2B**
//! (Google's Matformer audio + vision + text model, ~2 B effective params),
//! and streams the assistant's text response to stdout. Multi-turn — the
//! conversation history is preserved across recordings.
//!
//! ### Why Gemma 3n E2B?
//! Of the audio-capable architectures in `mistralrs 0.8.1`'s
//! `MultimodalLoaderType` (`Phi4MM`, `Voxtral`, `MiniCpmO`, `Gemma3n`),
//! Voxtral fails to load (`audio_config.json` is missing the `frame_rate`
//! field the 0.8.1 loader requires), Phi-4-MM and MiniCPM-o are too heavy
//! for a 16 GB machine once you add the audio encoder + KV cache on top of
//! the quantised weights. Gemma 3n E2B is the smallest natively-audio model
//! the 0.8.1 multimodal loader supports.
//!
//! ### macOS notes
//! - On first launch macOS will ask for microphone permission. Grant it in
//!   System Settings → Privacy & Security → Microphone.
//! - Run with the Metal backend for sane speed:
//!     cargo run --release --example mistralrs-voice \
//!       --features "mistralrs stream chat-mistralrs/metal"
//!
//! Expect a few minutes on first run while ~6 GB of weights download into
//! `~/.cache/huggingface/`. With `IsqType::Q4K` the loaded model lives in
//! roughly 1.5–2 GB of RAM.
//!
//! UX:
//!   1. Press enter to start recording.
//!   2. Speak.
//!   3. Press enter again to stop.
//!   4. The model processes the clip, then streams its text response.
//!   5. Loop.
//!
//! The response does **not** stream in parallel with your speech — the model
//! emits text only after consuming the full audio clip. Token-streaming
//! begins immediately once decoding starts.

use std::io::{self, BufRead, Cursor, Write};
use std::sync::{Arc, Mutex};

use chat_core::traits::StreamProvider;
use chat_core::types::messages::Messages;
use chat_core::types::messages::content::{CompleteReasonEnum, Content, RoleEnum};
use chat_core::types::messages::file::File;
use chat_core::types::messages::parts::{PartEnum, Parts};
use chat_core::types::messages::text::Text;
use chat_core::types::response::StreamEvent;
use chat_mistralrs::{IsqType, MistralRsBuilder};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use futures::StreamExt;
use hound::{SampleFormat, WavSpec, WavWriter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Loading Gemma 3n E2B (first run downloads ~6 GB, then stays warm)...");
    eprintln!("Progress from mistral.rs follows below. Watch for 'Model loaded' as the green light.\n");
    let mut client = MistralRsBuilder::new()
        .with_model("google/gemma-3n-E2B-it")
        .with_multimodal()
        .with_isq(IsqType::Q4K)
        .with_logging()
        .build()
        .await?;
    eprintln!("\nModel ready.\n");

    let host = cpal::default_host();
    let device = host.default_input_device().ok_or(
        "no default input device — check System Settings → Privacy & Security → Microphone",
    )?;
    let config = device.default_input_config()?;
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    eprintln!(
        "Microphone: {} @ {} Hz, {} channel(s), {:?}\n",
        device.name().unwrap_or_else(|_| "<unknown>".into()),
        sample_rate,
        channels,
        config.sample_format(),
    );

    let mut messages = Messages(vec![Content {
        role: RoleEnum::System,
        parts: Parts(vec![PartEnum::Text(Text::new(
            "You are a helpful voice assistant. For every user turn the user \
             sends an audio clip. Respond in EXACTLY this two-line format and \
             nothing else before it:\n\
             Heard: \"<verbatim transcript of what the user said>\"\n\
             Reply: <your answer — at least two full sentences, and engage \
             with the content of what was said; never reply with just \
             \"okay\" or other one-word acknowledgements>",
        ))]),
        complete_reason: CompleteReasonEnum::None,
    }]);
    let stdin = io::stdin();

    loop {
        // --- Wait for user to start ---
        prompt("Press enter to start recording (Ctrl+C to quit) ")?;
        if read_line(&stdin)?.is_none() {
            break;
        }

        // --- Capture mic into a shared f32 buffer ---
        let samples = Arc::new(Mutex::new(Vec::<f32>::with_capacity(sample_rate as usize * 30)));
        let stream = build_input_stream(&device, &config, samples.clone())?;
        stream.play()?;

        prompt("Recording... press enter to stop ")?;
        if read_line(&stdin)?.is_none() {
            break;
        }
        drop(stream); // stops capture

        let captured: Vec<f32> = {
            let guard = samples.lock().unwrap();
            guard.clone()
        };
        if captured.is_empty() {
            eprintln!("(no audio captured — try again)\n");
            continue;
        }
        let secs = captured.len() as f32 / sample_rate as f32 / channels as f32;
        eprintln!("Captured {secs:.1}s of audio. Encoding...");

        // --- Wrap as a chat-rs File part ---
        let wav_bytes = encode_wav(&captured, sample_rate, channels)?;
        let user_parts = Parts(vec![
            PartEnum::Text(Text::new(
                "Transcribe the attached audio verbatim on the `Heard:` line, \
                 then give a substantive reply on the `Reply:` line.",
            )),
            PartEnum::File(File::from_bytes_with_mime(wav_bytes, "audio/wav")),
        ]);
        messages.push(Content {
            role: RoleEnum::User,
            parts: user_parts,
            complete_reason: CompleteReasonEnum::None,
        });

        // --- Stream response to stdout, accumulate for history ---
        eprint!("\nAssistant: ");
        io::stderr().flush()?;
        let mut stream = client.stream(&mut messages, None, None).await?;
        let mut assistant_text = String::new();
        while let Some(event) = stream.next().await {
            match event? {
                StreamEvent::TextChunk(s) => {
                    print!("{s}");
                    io::stdout().flush()?;
                    assistant_text.push_str(&s);
                }
                StreamEvent::Done(_) => {
                    println!("\n");
                    break;
                }
                _ => {}
            }
        }

        // --- Push assistant turn for multi-turn context ---
        messages.push(Content {
            role: RoleEnum::Model,
            parts: Parts(vec![PartEnum::Text(Text::new(assistant_text))]),
            complete_reason: CompleteReasonEnum::Stop,
        });
    }

    Ok(())
}

fn prompt(text: &str) -> io::Result<()> {
    eprint!("{text}");
    io::stderr().flush()
}

fn read_line(stdin: &io::Stdin) -> io::Result<Option<String>> {
    let mut line = String::new();
    let n = stdin.lock().read_line(&mut line)?;
    if n == 0 { Ok(None) } else { Ok(Some(line)) }
}

fn encode_wav(
    samples: &[f32],
    sample_rate: u32,
    channels: u16,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    let mut cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut writer = WavWriter::new(&mut cursor, spec)?;
        for s in samples {
            writer.write_sample(*s)?;
        }
        writer.finalize()?;
    }
    Ok(cursor.into_inner())
}

fn build_input_stream(
    device: &cpal::Device,
    config: &cpal::SupportedStreamConfig,
    sink: Arc<Mutex<Vec<f32>>>,
) -> Result<cpal::Stream, Box<dyn std::error::Error>> {
    let err_fn = |e| eprintln!("mic stream error: {e}");
    let cfg = config.config();
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &cfg,
            move |data: &[f32], _| sink.lock().unwrap().extend_from_slice(data),
            err_fn,
            None,
        )?,
        cpal::SampleFormat::I16 => device.build_input_stream(
            &cfg,
            move |data: &[i16], _| {
                let mut buf = sink.lock().unwrap();
                buf.extend(data.iter().map(|s| *s as f32 / i16::MAX as f32));
            },
            err_fn,
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_input_stream(
            &cfg,
            move |data: &[u16], _| {
                let mut buf = sink.lock().unwrap();
                buf.extend(data.iter().map(|s| (*s as f32 - 32768.0) / 32768.0));
            },
            err_fn,
            None,
        )?,
        fmt => return Err(format!("unsupported sample format: {fmt:?}").into()),
    };
    Ok(stream)
}
