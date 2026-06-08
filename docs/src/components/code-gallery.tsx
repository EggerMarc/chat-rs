import { useEffect, useRef, useState } from "react";
import { codeToTokens } from "shiki";
import { Card } from "@/components/ui/card";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { CopyableCode } from "@/components/copyable-code";

/**
 * A self-cycling gallery of real chat-rs snippets across a mix of providers.
 *
 * Transitions are a full typewriter: the entire current snippet types out, then
 * the entire next one types in. Each snippet is highlighted once with Shiki; the
 * typewriter just reveals a prefix of the pre-tokenized output.
 *
 * Auto-advance shows a progress fill inside the active tab. Selecting a tab
 * manually stops the auto-advance entirely: it transitions to that snippet
 * and then sits still.
 */

type Snippet = { id: string; label: string; code: string };

type Char = { ch: string; style: Record<string, string> };

/** What's currently on screen: the first `visible` chars of `id`. */
type RenderState = { id: string; visible: number };

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
    // A local model, pulled and run on your machine. No API keys.
    let client = OllamaBuilder::new().with_model("llama3.2").pull().await?.build();
    let mut chat = ChatBuilder::new().with_model(client).build();

    let mut messages = messages::from_user(parts!["Explain ownership in one line."]);

    // Interaction is a typed event stream. Consume it as it arrives.
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
    label: "Tools",
    code: `use chat_rs::{ChatBuilder, ChatOutcome, claude::ClaudeBuilder, parts, types::messages};
use tools_rs::{collect_tools, tool};

#[tool]
/// Look up the current weather for a city.
async fn get_weather(city: String) -> String {
    format!("It's 22°C and sunny in {city}.")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = ClaudeBuilder::new().with_model("claude-sonnet-4").build();
    let mut chat = ChatBuilder::new()
        .with_tools(collect_tools())
        .with_model(client)
        .with_max_steps(5)
        .build();

    let mut messages = messages::from_user(parts!["What's the weather in Lisbon?"]);

    // The loop calls your function, feeds the result back, and continues.
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
    label: "Structured",
    code: `use chat_rs::{ChatBuilder, ChatOutcome, openai::OpenAIBuilder, parts, types::messages};
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(JsonSchema, Deserialize, Debug)]
struct Recipe {
    title: String,
    ingredients: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenAIBuilder::new().with_model("gpt-4o").build();

    // The model is constrained to your type. You get Recipe back, typed.
    let mut chat = ChatBuilder::new()
        .with_structured_output::<Recipe>()
        .with_model(client)
        .build();

    let mut messages = messages::from_user(parts!["A simple recipe for pancakes."]);

    if let ChatOutcome::Complete(res) = chat.complete(&mut messages).await.map_err(|e| e.err)? {
        println!("{}: {} ingredients", res.content.title, res.content.ingredients.len());
    }
    Ok(())
}`,
  },
];

const codeOf = (id: string) => SNIPPETS.find((s) => s.id === id)!.code;

export function CodeGallery() {
  const [chars, setChars] = useState<Record<string, Char[]>>({});
  const ready = Object.keys(chars).length === SNIPPETS.length;

  const [index, setIndex] = useState(0); // target snippet
  const [auto, setAuto] = useState(true);
  const [dwelling, setDwelling] = useState(false);
  const [render, setRender] = useState<RenderState>({
    id: SNIPPETS[0].id,
    visible: SNIPPETS[0].code.length,
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

  // Driver: type the current snippet out in full, then type the next in full.
  useEffect(() => {
    if (!ready) return;
    const fromId = renderRef.current.id;
    const toId = SNIPPETS[index].id;

    if (fromId === toId) {
      setRender({ id: toId, visible: codeOf(toId).length });
      if (autoRef.current) setDwelling(true);
      return;
    }

    setDwelling(false);
    const target = codeOf(toId).length;
    let phase: "out" | "in" = "out";
    let visible = codeOf(fromId).length;
    let srcId = fromId;

    const timer = setInterval(() => {
      if (phase === "out") {
        visible -= OUT_STEP;
        if (visible <= 0) {
          visible = 0;
          phase = "in";
          srcId = toId;
        }
      } else {
        visible += IN_STEP;
        if (visible >= target) visible = target;
      }
      setRender({ id: srcId, visible });

      if (phase === "in" && visible >= target) {
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

  // Compose the visible characters: the first `visible` chars of the snippet.
  const flat = chars[render.id];
  const runs: { text: string; style: Record<string, string> }[] = [];
  if (flat) {
    for (const c of flat.slice(0, render.visible)) {
      const last = runs[runs.length - 1];
      if (last && last.style === c.style) last.text += c.ch;
      else runs.push({ text: c.ch, style: { ...c.style } });
    }
  }

  const activeId = SNIPPETS[index].id;

  return (
    <Card className="gap-0 border border-border p-0 ring-0">
      <CopyableCode code={SNIPPETS[index].code}>
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
      </CopyableCode>

      {/* Bottom tabs (shadcn / Base UI). The progress fill loads inside the
          selected tab only; selecting a tab stops the auto-advance. */}
      <Tabs
        value={activeId}
        onValueChange={(value) =>
          select(SNIPPETS.findIndex((s) => s.id === value))
        }
        className="gap-0 border-t border-border"
      >
        <TabsList
          variant="line"
          className="flex h-auto w-full flex-wrap bg-transparent p-0"
        >
          {SNIPPETS.map((s) => (
            <TabsTrigger
              key={s.id}
              value={s.id}
              className="font-label relative h-auto flex-1 overflow-hidden px-4 py-3 text-xs uppercase tracking-[0.2em] data-active:text-primary"
            >
              {auto && dwelling && s.id === activeId && (
                <span
                  className="pointer-events-none absolute inset-y-0 left-0 bg-primary/15"
                  style={{
                    animation: `gallery-sweep ${DWELL_MS}ms linear forwards`,
                  }}
                  onAnimationEnd={() => {
                    setDwelling(false);
                    if (autoRef.current)
                      setIndex((idx) => (idx + 1) % SNIPPETS.length);
                  }}
                />
              )}
              <span className="relative">{s.label}</span>
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>
    </Card>
  );
}
