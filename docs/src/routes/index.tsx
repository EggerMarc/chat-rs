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

const PRIMITIVES = [
  {
    label: "Messages",
    body: "The unit of exchange. Roles, turns, and ordering, consistent across every provider.",
  },
  {
    label: "Content",
    body: "Text, images, audio, video, documents, and structured data — text is never privileged.",
  },
  {
    label: "Tools",
    body: "Definitions, invocations, and results. The mechanism, not the policy.",
  },
  {
    label: "Streams",
    body: "Interaction as an event stream — produced and consumed incrementally.",
  },
  {
    label: "Events",
    body: "One StreamEvent surface for text, reasoning, tool calls, and structured output.",
  },
  {
    label: "Embeddings",
    body: "Vector representations exposed as a first-class capability of a provider.",
  },
];

const MODEL = [
  {
    label: "Streaming",
    title: "Streaming is the core interaction model",
    body: "Not an optional feature. A model may emit text, reasoning, tool calls, structured outputs, or multimodal content; applications may supply information incrementally rather than as a single request. The same model serves conversational, voice, robotic, and realtime systems without new abstractions.",
  },
  {
    label: "Multimodal",
    title: "No privileged data type",
    body: "Messages carry multiple forms of content. Provider-specific representations are translated into a common internal one, so applications reason about content consistently — without provider formats leaking into your code.",
  },
  {
    label: "Tools",
    title: "Tool execution is model interaction",
    body: "A model may request information or actions; the application decides which tools exist and how they run. The runtime represents definitions, invocations, and results — it does not define planners, retries, or approval systems.",
  },
  {
    label: "Providers",
    title: "Capabilities, not vendors",
    body: "Reason about streaming, embeddings, structured outputs, multimodal inputs, and tool calling without coupling to a specific vendor. Integrations preserve a stable programming model as providers evolve.",
  },
];

const IS = [
  "model interaction",
  "content representation",
  "streaming",
  "events",
  "tool invocation",
  "embeddings",
  "provider integration",
  "capability abstraction",
];

const IS_NOT = [
  "agent frameworks",
  "workflow engines",
  "DAG execution",
  "planning systems",
  "memory architectures",
  "vector storage",
  "business logic",
  "application state",
];

function Eyebrow({ children }: { children: React.ReactNode }) {
  return (
    <p className="font-label text-xs uppercase tracking-[0.2em] text-fd-muted-foreground">
      {children}
    </p>
  );
}

function SectionBar({ index, title }: { index: string; title: string }) {
  return (
    <div className="border-b border-fd-border px-4 py-3 sm:px-6">
      <h2 className="font-label flex items-baseline gap-3 text-lg sm:text-2xl">
        <span className="text-fd-primary">{index}</span>
        <span className="tracking-tight text-fd-foreground">{title}</span>
      </h2>
    </div>
  );
}

function Home() {
  return (
    <HomeLayout {...baseOptions()}>
      <main className="mx-auto flex w-full max-w-5xl flex-1 flex-col border-x border-fd-border">
        {/* Hero */}
        <section className="flex flex-col gap-6 px-4 py-20 sm:px-6 sm:py-28">
          <Eyebrow>A Creology runtime · Rust</Eyebrow>
          <h1 className="max-w-3xl text-balance text-3xl font-medium leading-[1.1] tracking-tight sm:text-5xl">
            A runtime for model interaction.
          </h1>
          <p className="max-w-2xl text-base leading-relaxed text-fd-muted-foreground">
            <span className="text-fd-foreground">chat-rs</span> provides one
            consistent set of abstractions for working with language models and
            other intelligent systems — regardless of provider, modality, or
            deployment target. It is the interaction layer, and nothing above
            it.
          </p>
          <div className="flex flex-wrap items-center gap-3 pt-2">
            <Link
              to="/docs/$"
              params={{ _splat: "getting-started" }}
              className="inline-flex items-center border border-fd-primary px-5 py-2.5 text-sm font-medium text-fd-primary transition-colors hover:bg-fd-primary hover:text-fd-primary-foreground"
            >
              Get started
            </Link>
            <a
              href="https://github.com/EggerMarc/chat-rs"
              className="inline-flex items-center border border-fd-border px-5 py-2.5 text-sm font-medium text-fd-foreground transition-colors hover:bg-fd-accent"
            >
              View on GitHub
            </a>
          </div>
        </section>

        {/* Code sample */}
        <section className="border-t border-fd-border bg-fd-card">
          <div className="flex items-center gap-2 border-b border-fd-border px-4 py-2.5 sm:px-6">
            <span className="font-label text-xs uppercase tracking-[0.2em] text-fd-muted-foreground">
              main.rs
            </span>
          </div>
          <pre className="overflow-x-auto px-4 py-5 text-sm leading-relaxed sm:px-6">
            <code>{SAMPLE}</code>
          </pre>
        </section>

        {/* 01 — Primitives */}
        <section className="border-t border-fd-border">
          <SectionBar index="01" title="The primitives" />
          <p className="max-w-2xl px-4 pt-6 text-sm leading-relaxed text-fd-muted-foreground sm:px-6">
            Model interaction is a more stable abstraction than most
            application-level AI concepts. chat-rs focuses on the lower-level
            concepts that appear regardless of orchestration strategy, and makes
            them available in a consistent, composable way.
          </p>
          <div className="mt-6 grid grid-cols-1 gap-px border-t border-fd-border bg-fd-border sm:grid-cols-2 lg:grid-cols-3">
            {PRIMITIVES.map((p, i) => (
              <div key={p.label} className="bg-fd-background p-5 sm:p-6">
                <div className="mb-3 flex items-baseline gap-2">
                  <span className="font-label text-xs text-fd-primary">
                    {String(i + 1).padStart(2, "0")}
                  </span>
                  <h3 className="font-label text-xs uppercase tracking-[0.2em] text-fd-muted-foreground">
                    {p.label}
                  </h3>
                </div>
                <p className="text-sm leading-relaxed text-fd-foreground">
                  {p.body}
                </p>
              </div>
            ))}
          </div>
        </section>

        {/* 02 — Interaction model */}
        <section className="border-t border-fd-border">
          <SectionBar index="02" title="The interaction model" />
          <div className="grid grid-cols-1 gap-px bg-fd-border lg:grid-cols-2">
            {MODEL.map((m, i) => (
              <div key={m.label} className="bg-fd-background p-5 sm:p-8">
                <div className="mb-4 flex items-baseline gap-2">
                  <span className="font-label text-xs text-fd-primary">
                    {String(i + 1).padStart(2, "0")}
                  </span>
                  <Eyebrow>{m.label}</Eyebrow>
                </div>
                <h3 className="mb-2 text-base font-semibold tracking-tight">
                  {m.title}
                </h3>
                <p className="max-w-md text-sm leading-relaxed text-fd-muted-foreground">
                  {m.body}
                </p>
              </div>
            ))}
          </div>
        </section>

        {/* 03 — Scope */}
        <section className="border-t border-fd-border">
          <SectionBar index="03" title="Scope" />
          <div className="grid grid-cols-1 gap-px bg-fd-border md:grid-cols-2">
            <div className="bg-fd-background p-5 sm:p-8">
              <Eyebrow>Responsible for</Eyebrow>
              <ul className="mt-5 space-y-2.5">
                {IS.map((item, i) => (
                  <li key={item} className="flex items-baseline gap-3 text-sm">
                    <span className="font-label text-xs text-fd-primary">
                      {String(i + 1).padStart(2, "0")}
                    </span>
                    <span className="text-fd-foreground">{item}</span>
                  </li>
                ))}
              </ul>
            </div>
            <div className="bg-fd-background p-5 sm:p-8">
              <Eyebrow>Not responsible for</Eyebrow>
              <ul className="mt-5 space-y-2.5">
                {IS_NOT.map((item, i) => (
                  <li key={item} className="flex items-baseline gap-3 text-sm">
                    <span className="font-label text-xs text-fd-muted-foreground">
                      {String(i + 1).padStart(2, "0")}
                    </span>
                    <span className="text-fd-muted-foreground line-through decoration-fd-border">
                      {item}
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          </div>
          <p className="max-w-2xl px-4 py-6 text-sm leading-relaxed text-fd-muted-foreground sm:px-6">
            These systems can be built using chat-rs, but they do not belong to
            the runtime itself. The preferred approach is to strengthen existing
            primitives rather than introduce new layers of orchestration.
          </p>
        </section>

        {/* CTA */}
        <section className="border-t border-fd-border px-4 py-16 sm:px-6">
          <Eyebrow>Start here</Eyebrow>
          <h2 className="mt-4 max-w-2xl text-balance text-2xl font-medium tracking-tight sm:text-3xl">
            A stable programming model as providers evolve.
          </h2>
          <div className="mt-6 flex flex-wrap items-center gap-3">
            <Link
              to="/docs/$"
              params={{ _splat: "getting-started" }}
              className="inline-flex items-center border border-fd-primary px-5 py-2.5 text-sm font-medium text-fd-primary transition-colors hover:bg-fd-primary hover:text-fd-primary-foreground"
            >
              Read the docs
            </Link>
            <a
              href="https://github.com/EggerMarc/chat-rs"
              className="inline-flex items-center border border-fd-border px-5 py-2.5 text-sm font-medium text-fd-foreground transition-colors hover:bg-fd-accent"
            >
              View on GitHub
            </a>
          </div>
        </section>

        {/* Colophon */}
        <footer className="flex flex-col gap-4 border-t border-fd-border px-4 py-10 sm:flex-row sm:items-center sm:justify-between sm:px-6">
          <span className="font-label text-xs uppercase tracking-[0.2em] text-fd-muted-foreground">
            Open source · MIT
          </span>
          <a
            href="https://creology.co"
            className="inline-flex items-center gap-2 text-fd-muted-foreground transition-opacity hover:opacity-80"
          >
            <span className="font-label text-xs uppercase tracking-[0.2em]">
              Built by
            </span>
            <img
              src="/logo-light.webp"
              alt="Creology"
              className="brand-on-dark h-6 w-auto"
            />
            <img
              src="/logo-dark.webp"
              alt="Creology"
              className="brand-on-light h-6 w-auto"
            />
          </a>
        </footer>
      </main>
    </HomeLayout>
  );
}
