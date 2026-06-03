import { createFileRoute, Link } from "@tanstack/react-router";
import { HomeLayout } from "fumadocs-ui/layouts/home";
import { baseOptions } from "@/lib/layout.shared";
import { CodeGallery } from "@/components/code-gallery";

export const Route = createFileRoute("/")({
  component: Home,
});

const FEATURES: {
  label: string;
  title: string;
  body: string;
  tag?: string;
}[] = [
  {
    label: "Providers",
    title: "One API, every provider",
    body: "OpenAI, Claude, Gemini, Ollama, DeepSeek, OpenRouter and more. Change one builder and your call sites never move.",
  },
  {
    label: "Multimodal",
    title: "Every modality is a Part",
    body: "Text, images, audio, video, and documents mix in a single message, with no per-type plumbing.",
  },
  {
    label: "Images",
    title: "Generate images, too",
    tag: "Beta",
    body: "Ask a capable model for pictures, not just words. The same call returns image parts alongside text.",
  },
  {
    label: "Duplex",
    title: "Talk back mid-stream",
    body: "Push new input while the model is still responding. It merges into context and the model carries on. True full duplex.",
  },
  {
    label: "Tools",
    title: "Tools your model can call",
    body: "Annotate an async fn with #[tool]. The loop runs it, returns the result, and can pause for a human to approve.",
  },
  {
    label: "Structured",
    title: "Typed output, every time",
    body: "Constrain any model to a Rust type and get it back deserialized. No parsing, no guesswork, no surprises at runtime.",
  },
  {
    label: "Type-state",
    title: "The compiler checks your wiring",
    body: "Missing configuration is a compile error, not a 2am panic. Structured chats return your type; streamed chats stream.",
  },
  {
    label: "Responses",
    title: "One response shape, always",
    body: "complete, resume, and stream return the same content and metadata, so you can switch modes without a rewrite.",
  },
  {
    label: "Routing",
    title: "Route and recover",
    body: "Send each request to the right model by cost or capability, with automatic fallback when a provider is down.",
  },
];

const IN_BOX = [
  "Messages & multimodal content",
  "Streaming & duplex input",
  "Tools & human-in-the-loop",
  "Structured output & embeddings",
  "Provider routing & fallback",
  "Pluggable HTTP / WebSocket transport",
];

const BRING_YOUR_OWN = [
  "Agents & planners",
  "Workflows & DAGs",
  "Memory & vector stores",
  "Business logic",
  "Application state",
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
    <div className="border-b border-fd-border px-4 py-4 sm:px-6 sm:py-5">
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
        <section className="flex flex-col gap-7 px-4 py-24 sm:px-6 sm:py-36">
          <Eyebrow>A Creology runtime · Rust</Eyebrow>
          <h1 className="max-w-3xl text-balance text-3xl font-medium leading-[1.1] tracking-tight sm:text-5xl">
            One Rust API for every model.
          </h1>
          <p className="max-w-2xl text-balance text-base leading-relaxed text-fd-muted-foreground">
            <span className="text-fd-foreground">chat-rs</span> is the
            interaction layer between your app and any language model. Streaming,
            multimodal content, tools, structured output, and multi-provider
            routing, behind one type-safe API across OpenAI, Claude, Gemini,
            Ollama, and more.
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
            <span className="inline-flex items-center gap-2.5 border border-fd-border bg-fd-card px-4 py-2.5">
              <span className="font-label text-xs text-fd-primary">$</span>
              <code className="text-sm">cargo add chat-rs</code>
            </span>
          </div>
        </section>

        {/* Code gallery */}
        <section className="border-t border-fd-border px-4 py-12 sm:px-6 sm:py-16">
          <CodeGallery />
        </section>

        {/* 01 - Features */}
        <section className="border-t border-fd-border">
          <SectionBar index="01" title="Everything you need to talk to models" />
          <p className="max-w-2xl px-4 pt-8 text-sm leading-relaxed text-fd-muted-foreground sm:px-6">
            From a one-line completion to streaming, tools, and typed output,
            chat-rs hands you the whole interaction surface and keeps it
            identical across every provider you reach for.
          </p>
          <div className="mt-8 grid grid-cols-1 gap-px border-t border-fd-border bg-fd-border sm:grid-cols-2 lg:grid-cols-3">
            {FEATURES.map((f, i) => (
              <div key={f.label} className="bg-fd-background p-6 sm:p-8">
                <div className="mb-4 flex items-center gap-2">
                  <span className="font-label text-xs text-fd-primary">
                    {String(i + 1).padStart(2, "0")}
                  </span>
                  <span className="font-label text-xs uppercase tracking-[0.2em] text-fd-muted-foreground">
                    {f.label}
                  </span>
                  {f.tag && (
                    <span className="font-label ml-auto border border-fd-primary px-1.5 py-0.5 text-[10px] uppercase tracking-[0.15em] text-fd-primary">
                      {f.tag}
                    </span>
                  )}
                </div>
                <h3 className="mb-2 text-base font-semibold tracking-tight">
                  {f.title}
                </h3>
                <p className="text-sm leading-relaxed text-fd-muted-foreground">
                  {f.body}
                </p>
              </div>
            ))}
          </div>
        </section>

        {/* 02 - Small on purpose */}
        <section className="border-t border-fd-border">
          <SectionBar index="02" title="Small on purpose" />
          <p className="max-w-2xl px-4 pt-8 text-sm leading-relaxed text-fd-muted-foreground sm:px-6">
            chat-rs is the runtime, not the framework. You get a rock-solid
            interaction layer; agents, workflows, and memory stay yours to build
            on top, however you like, with whatever architecture fits.
          </p>
          <div className="mt-8 grid grid-cols-1 gap-px border-t border-fd-border bg-fd-border md:grid-cols-2">
            <div className="bg-fd-background p-6 sm:p-10">
              <Eyebrow>In the box</Eyebrow>
              <ul className="mt-6 space-y-3">
                {IN_BOX.map((item) => (
                  <li key={item} className="flex items-baseline gap-3 text-sm">
                    <span className="font-label text-xs text-fd-primary">+</span>
                    <span className="text-fd-foreground">{item}</span>
                  </li>
                ))}
              </ul>
            </div>
            <div className="bg-fd-background p-6 sm:p-10">
              <Eyebrow>Bring your own</Eyebrow>
              <ul className="mt-6 space-y-3">
                {BRING_YOUR_OWN.map((item) => (
                  <li key={item} className="flex items-baseline gap-3 text-sm">
                    <span className="font-label text-xs text-fd-muted-foreground">
                      ·
                    </span>
                    <span className="text-fd-muted-foreground">{item}</span>
                  </li>
                ))}
              </ul>
            </div>
          </div>
        </section>

        {/* CTA */}
        <section className="border-t border-fd-border px-4 py-20 sm:px-6 sm:py-28">
          <Eyebrow>Start here</Eyebrow>
          <h2 className="mt-5 max-w-2xl text-balance text-2xl font-medium tracking-tight sm:text-3xl">
            Ship your first completion in five minutes.
          </h2>
          <p className="mt-5 max-w-xl text-sm leading-relaxed text-fd-muted-foreground">
            Add the crate, pick a provider, and call <code>complete</code>. Swap
            in streaming, tools, or routing whenever you're ready. The API stays
            the same.
          </p>
          <div className="mt-7 flex flex-wrap items-center gap-3">
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
        <footer className="flex flex-col gap-4 border-t border-fd-border px-4 py-12 sm:flex-row sm:items-center sm:justify-between sm:px-6 sm:py-16">
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
