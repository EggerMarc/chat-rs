import { useEffect, useRef, useState } from "react";
import { codeToTokens } from "shiki";

/**
 * A self-cycling gallery of real chat-rs snippets across a mix of providers.
 *
 * Transitions are a diff-aware typewriter: the shared leading/trailing text
 * between two snippets (imports, boilerplate) stays put, and only the differing
 * middle is typed out and re-typed in. Each snippet is highlighted once with
 * Shiki; the typewriter just reveals a slice of the pre-tokenized output.
 *
 * Auto-advance shows a progress fill inside the active tab. Selecting a tab
 * manually stops the auto-advance entirely — it transitions to that snippet
 * and then sits still.
 */

type Snippet = { id: string; label: string; code: string };

type Char = { ch: string; style: Record<string, string> };

/** What's currently on screen: prefix + middle(mid chars) + suffix of `id`. */
type RenderState = { id: string; prefix: number; suffix: number; mid: number };

const OUT_STEP = 9; // chars deleted per tick
const IN_STEP = 4; // chars typed per tick
const TICK_MS = 16;
const DWELL_MS = 3500; // pause on a settled snippet before auto-advancing

const SNIPPETS: Snippet[] = [
  {
    id: "streaming",
    label: "Streaming",
    code: `use chat_rs::{ChatBuilder, StreamEvent, ollama::OllamaBuilder, parts, types::messages};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A local model, pulled and run on your machine — no API keys.
    let client = OllamaBuilder::new()
        .with_model("llama3.2")
        .pull()
        .await?
        .build();

    let mut chat = ChatBuilder::new().with_model(client).build();
    let mut messages = messages::from_user(parts!["Explain ownership in one line."]);

    // Interaction is an event stream — consume it as it arrives.
    let mut stream = chat.stream(&mut messages).await.map_err(|e| e.err)?;
    while let Some(event) = stream.next().await {
        if let Ok(StreamEvent::TextChunk(text)) = event {
            print!("{text}");
        }
    }
    Ok(())
}`,
  },
  {
    id: "tools",
    label: "Tool calling",
    code: `use chat_rs::{ChatBuilder, ChatOutcome, openai::OpenAIBuilder, parts, types::messages};
use tools_rs::{collect_tools, tool};

#[tool]
/// Look up the current weather for a city.
async fn get_weather(city: String) -> String {
    format!("It's 22°C and sunny in {city}.")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenAIBuilder::new().with_model("gpt-4o").build();

    let mut chat = ChatBuilder::new()
        .with_tools(collect_tools())
        .with_model(client)
        .with_max_steps(5)
        .build();

    let mut messages = messages::from_user(parts!["What's the weather in Lisbon?"]);

    // The loop runs tools until the model has nothing left to call.
    if let ChatOutcome::Complete(res) = chat.complete(&mut messages).await.map_err(|e| e.err)? {
        if let Some(text) = res.content.parts.text_response() {
            println!("{text}");
        }
    }
    Ok(())
}`,
  },
  {
    id: "structured",
    label: "Structured output",
    code: `use chat_rs::{ChatBuilder, ChatOutcome, gemini::GeminiBuilder, parts, types::messages};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(JsonSchema, Deserialize, Debug)]
struct Recipe {
    title: String,
    ingredients: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = GeminiBuilder::new().with_model("gemini-2.5-flash".to_string()).build();

    // The schema is sent to the model; the reply deserializes into Recipe.
    let mut chat = ChatBuilder::new()
        .with_structured_output::<Recipe>()
        .with_model(client)
        .build();

    let mut messages = messages::from_user(parts!["A simple recipe for pancakes."]);

    if let ChatOutcome::Complete(res) = chat.complete(&mut messages).await.map_err(|e| e.err)? {
        println!("{} — {} ingredients", res.content.title, res.content.ingredients.len());
    }
    Ok(())
}`,
  },
  {
    id: "routing",
    label: "Routing",
    code: `use chat_rs::{
    ChatBuilder, ChatOutcome, claude::ClaudeBuilder, ollama::OllamaBuilder, parts,
    router::RouterBuilder, types::messages,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let local = OllamaBuilder::new().with_model("llama3.2").build();
    let cloud = ClaudeBuilder::new().with_model("claude-sonnet-4".to_string()).build();

    // No strategy = try providers in order, falling back on failure.
    // Add .with_strategy(..) to route by capability, cost, or keywords.
    let router = RouterBuilder::new()
        .add_provider(local)
        .add_provider(cloud)
        .build();

    let mut chat = ChatBuilder::new().with_model(router).build();
    let mut messages = messages::from_user(parts!["Draft a haiku about Rust."]);

    if let ChatOutcome::Complete(res) = chat.complete(&mut messages).await.map_err(|e| e.err)? {
        if let Some(text) = res.content.parts.text_response() {
            println!("{text}");
        }
    }
    Ok(())
}`,
  },
];

const codeOf = (id: string) => SNIPPETS.find((s) => s.id === id)!.code;

function commonPrefix(a: string, b: string): number {
  const n = Math.min(a.length, b.length);
  let i = 0;
  while (i < n && a[i] === b[i]) i++;
  return i;
}

function commonSuffix(a: string, b: string, prefix: number): number {
  const max = Math.min(a.length, b.length) - prefix;
  let i = 0;
  while (i < max && a[a.length - 1 - i] === b[b.length - 1 - i]) i++;
  return i;
}

export function CodeGallery() {
  const [chars, setChars] = useState<Record<string, Char[]>>({});
  const ready = Object.keys(chars).length === SNIPPETS.length;

  const [index, setIndex] = useState(0); // target snippet
  const [auto, setAuto] = useState(true);
  const [dwelling, setDwelling] = useState(false);
  const [render, setRender] = useState<RenderState>({
    id: SNIPPETS[0].id,
    prefix: 0,
    suffix: 0,
    mid: SNIPPETS[0].code.length,
  });

  const renderRef = useRef(render);
  renderRef.current = render;
  const autoRef = useRef(auto);
  autoRef.current = auto;

  // Pre-tokenize every snippet once, dual-theme so it tracks light/dark.
  useEffect(() => {
    let alive = true;
    (async () => {
      const entries = await Promise.all(
        SNIPPETS.map(async (s) => {
          const { tokens } = await codeToTokens(s.code, {
            lang: "rust",
            themes: { light: "github-light", dark: "github-dark" },
            defaultColor: false,
          });
          const flat: Char[] = [];
          tokens.forEach((line, li) => {
            for (const tok of line) {
              const style = (tok.htmlStyle ?? {}) as Record<string, string>;
              for (const ch of tok.content) flat.push({ ch, style });
            }
            if (li < tokens.length - 1) flat.push({ ch: "\n", style: {} });
          });
          return [s.id, flat] as const;
        }),
      );
      if (alive) setChars(Object.fromEntries(entries));
    })();
    return () => {
      alive = false;
    };
  }, []);

  // Driver: animate the transition into `index`, then dwell if still auto.
  useEffect(() => {
    if (!ready) return;
    const fromId = renderRef.current.id;
    const toId = SNIPPETS[index].id;

    if (fromId === toId) {
      const len = codeOf(toId).length;
      setRender({ id: toId, prefix: 0, suffix: 0, mid: len });
      if (autoRef.current) setDwelling(true);
      return;
    }

    setDwelling(false);
    const a = codeOf(fromId);
    const b = codeOf(toId);
    const prefix = commonPrefix(a, b);
    const suffix = commonSuffix(a, b, prefix);
    const aMid = a.length - prefix - suffix;
    const bMid = b.length - prefix - suffix;

    let phase: "out" | "in" = aMid > 0 ? "out" : "in";
    let mid = aMid;
    let srcId = fromId;

    const timer = setInterval(() => {
      if (phase === "out") {
        mid -= OUT_STEP;
        if (mid <= 0) {
          mid = 0;
          phase = "in";
          srcId = toId;
        }
      } else {
        mid += IN_STEP;
        if (mid >= bMid) mid = bMid;
      }
      setRender({ id: srcId, prefix, suffix, mid });

      if (phase === "in" && mid >= bMid) {
        clearInterval(timer);
        if (autoRef.current) setDwelling(true);
      }
    }, TICK_MS);

    return () => clearInterval(timer);
  }, [index, ready]);

  const select = (i: number) => {
    setAuto(false);
    setDwelling(false);
    setIndex(i);
  };

  // Compose the visible characters: prefix + middle(mid) + suffix.
  const flat = chars[render.id];
  let runs: { text: string; style: Record<string, string> }[] = [];
  if (flat) {
    const len = flat.length;
    const visible: Char[] = [
      ...flat.slice(0, render.prefix),
      ...flat.slice(render.prefix, render.prefix + render.mid),
      ...flat.slice(len - render.suffix, len),
    ];
    for (const c of visible) {
      const last = runs[runs.length - 1];
      if (last && last.style === c.style) last.text += c.ch;
      else runs.push({ text: c.ch, style: { ...c.style } });
    }
  }

  return (
    <div className="border border-fd-border bg-fd-card">
      <pre className="shiki not-prose min-h-[27rem] overflow-x-auto px-4 py-5 text-sm leading-relaxed sm:px-6">
        <code>
          {flat
            ? runs.map((r, i) => (
                <span key={i} style={r.style as React.CSSProperties}>
                  {r.text}
                </span>
              ))
            : codeOf(render.id)}
          <span className="gallery-cursor" aria-hidden>
            ▍
          </span>
        </code>
      </pre>

      {/* Bottom tabs — the progress fill loads inside the selected tab only. */}
      <div className="flex flex-wrap border-t border-fd-border">
        {SNIPPETS.map((s, i) => (
          <button
            key={s.id}
            type="button"
            onClick={() => select(i)}
            aria-current={i === index}
            className={`font-label relative flex-1 overflow-hidden px-4 py-3 text-center text-xs uppercase tracking-[0.2em] transition-colors ${
              i === index
                ? "text-fd-primary"
                : "text-fd-muted-foreground hover:text-fd-foreground"
            }`}
          >
            {auto && dwelling && i === index && (
              <span
                className="pointer-events-none absolute inset-y-0 left-0 bg-fd-primary/15"
                style={{ animation: `gallery-sweep ${DWELL_MS}ms linear forwards` }}
                onAnimationEnd={() => {
                  setDwelling(false);
                  if (autoRef.current)
                    setIndex((idx) => (idx + 1) % SNIPPETS.length);
                }}
              />
            )}
            <span className="relative">{s.label}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
