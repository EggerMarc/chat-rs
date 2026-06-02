import { createFileRoute, Link } from "@tanstack/react-router";
import { HomeLayout } from "fumadocs-ui/layouts/home";
import { baseOptions } from "@/lib/layout.shared";

export const Route = createFileRoute("/")({
  component: Home,
});

const SAMPLE = `use chat_rs::{ChatBuilder, openrouter::OpenRouterBuilder, parts, types::messages};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = OpenRouterBuilder::new()
        .with_model("anthropic/claude-sonnet-4")
        .build();

    let mut chat = ChatBuilder::new().with_model(client).build();
    let mut msgs = messages::from_user(parts!["Say hello in one sentence."]);

    let res = chat.complete(&mut msgs).await?.expect_complete();
    println!("{:#?}", res.content);
    Ok(())
}`;

const FEATURES = [
  {
    title: "One API, many providers",
    body: "Swap between OpenAI, Claude, Gemini, OpenRouter, DeepSeek, Ollama and more without touching your call sites.",
  },
  {
    title: "Type-state builders",
    body: "Required configuration is enforced at compile time — misuse is a type error, not a runtime panic.",
  },
  {
    title: "Streaming first",
    body: "Token-by-token output over SSE (and WebSocket where supported) behind a single StreamEvent enum.",
  },
  {
    title: "Tools & human-in-the-loop",
    body: "Native tool calling with pause/resume flows for approvals and side-effecting actions.",
  },
  {
    title: "Pluggable transport",
    body: "Bring your own HTTP/WS transport, or use the built-in reqwest and tungstenite implementations.",
  },
  {
    title: "Composable wires",
    body: "Shared Chat Completions and Responses API wire crates — new OpenAI-compatible providers are thin wrappers.",
  },
];

function Home() {
  return (
    <HomeLayout {...baseOptions()}>
      <main className="flex flex-1 flex-col">
        <section className="mx-auto flex max-w-4xl flex-col items-center gap-6 px-4 py-24 text-center">
          <span className="rounded-full border border-fd-border px-3 py-1 text-sm text-fd-muted-foreground">
            Rust · LLM clients · type-safe
          </span>
          <h1 className="text-4xl font-bold tracking-tight sm:text-6xl">
            Build LLM clients with ease
          </h1>
          <p className="max-w-2xl text-lg text-fd-muted-foreground">
            <span className="font-semibold text-fd-foreground">chat-rs</span> is
            a unified, type-safe Rust framework for Large Language Models. One
            API across every major provider, with streaming, tools, and
            automatic retries.
          </p>
          <div className="flex flex-wrap items-center justify-center gap-3">
            <Link
              to="/docs/$"
              params={{ _splat: "getting-started" }}
              className="rounded-lg bg-fd-primary px-5 py-2.5 font-medium text-fd-primary-foreground transition-opacity hover:opacity-90"
            >
              Get started
            </Link>
            <a
              href="https://github.com/EggerMarc/chat-rs"
              className="rounded-lg border border-fd-border px-5 py-2.5 font-medium transition-colors hover:bg-fd-accent"
            >
              View on GitHub
            </a>
          </div>
        </section>

        <section className="mx-auto w-full max-w-3xl px-4 pb-16">
          <div className="overflow-hidden rounded-xl border border-fd-border bg-fd-card">
            <div className="flex items-center gap-1.5 border-b border-fd-border px-4 py-2.5">
              <span className="h-3 w-3 rounded-full bg-red-400" />
              <span className="h-3 w-3 rounded-full bg-yellow-400" />
              <span className="h-3 w-3 rounded-full bg-green-400" />
              <span className="ml-2 text-xs text-fd-muted-foreground">
                main.rs
              </span>
            </div>
            <pre className="overflow-x-auto p-4 text-sm leading-relaxed">
              <code>{SAMPLE}</code>
            </pre>
          </div>
        </section>

        <section className="mx-auto grid w-full max-w-5xl grid-cols-1 gap-4 px-4 pb-24 sm:grid-cols-2 lg:grid-cols-3">
          {FEATURES.map((f) => (
            <div
              key={f.title}
              className="rounded-xl border border-fd-border bg-fd-card p-5"
            >
              <h3 className="mb-1.5 font-semibold">{f.title}</h3>
              <p className="text-sm text-fd-muted-foreground">{f.body}</p>
            </div>
          ))}
        </section>
      </main>
    </HomeLayout>
  );
}
