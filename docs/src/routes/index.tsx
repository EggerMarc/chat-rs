import { createFileRoute, Link } from "@tanstack/react-router";
import { HomeLayout } from "fumadocs-ui/layouts/home";
import { baseOptions } from "@/lib/layout.shared";
import { CodeGallery } from "@/components/code-gallery";
import FaultyTerminal from "@/components/FaultyTerminal";
import { DynamicCodeBlock } from "fumadocs-ui/components/dynamic-codeblock";
import { useState } from "react";
import { Check, Copy } from "lucide-react";
import { buttonVariants } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { CopyableCode } from "@/components/copyable-code";
import { cn } from "@/lib/utils";

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
    body: "OpenAI, Claude, Gemini, Ollama, DeepSeek, OpenRouter and more. Change one builder; your call sites never move.",
  },
  {
    label: "Type-state",
    title: "The compiler checks your wiring",
    body: "Missing configuration is a compile error, not a 2am panic. The builder won't let you call what you didn't set up.",
  },
  {
    label: "Responses",
    title: "One response shape, always",
    body: "complete, resume, and stream return the same content and metadata. Switch modes without a rewrite.",
  },
  {
    label: "Tools",
    title: "Tools your model can call",
    body: "Annotate an async fn with #[tool]. The loop runs it, feeds the result back, and keeps going.",
  },
  {
    label: "Human in the loop",
    title: "Approve before it acts",
    body: "Pause the loop on a tool call, let a human approve, reject, or edit it, then resume right where it left off.",
  },
  {
    label: "Structured",
    title: "Typed output, every time",
    body: "Constrain any model to a Rust type and get it back deserialized. No parsing, no guesswork.",
  },
  {
    label: "Embeddings",
    title: "Embeddings, first-class",
    body: "Vectors come from the same providers through one embed call, ready for search and retrieval.",
  },
  {
    label: "Images",
    title: "Generate images, too",
    tag: "Beta",
    body: "Ask a capable model for pictures, not just words. Image parts come back alongside the text.",
  },
  {
    label: "Routing",
    title: "Route and recover",
    body: "Send each request to the right model by cost or capability, with automatic fallback when one is down.",
  },
];

const RUNTIME_SNIPPET = `use async_trait::async_trait;
use chat_rs::{
    ChatFailure, ChatResponse, CompletionProvider, Messages,
    types::{options::ChatOptions, tools::ToolDeclarations},
};

// A provider implements one trait. Bring a hosted API, a local model,
// or your own gateway. It need not speak Completions or Responses.
struct MyProvider;

#[async_trait]
impl CompletionProvider for MyProvider {
    async fn complete(
        &mut self,
        messages: &mut Messages,
        tools: Option<&dyn ToolDeclarations>,
        options: Option<&ChatOptions>,
        schema: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        // Call your model, then hand back a ChatResponse. chat-rs runs the
        // reason-and-act loop, tools, and retries for you.
        todo!()
    }
}`;

const RUNTIME_POINTS = [
  {
    title: "Plug in any provider",
    body: "A provider is just one trait. Bring a hosted API, a local model, or your own gateway. It doesn't have to speak Chat Completions or the Responses API.",
  },
  {
    title: "The loop is handled for you",
    body: "Streaming, retries, structured output, and human-in-the-loop pauses all come built in, so anything you plug in gets them for free.",
  },
  {
    title: "Native and custom tools, together",
    body: "Providers can expose their own native tools, and your #[tool] functions run right alongside them. Put a few providers behind a router and send each request wherever it fits.",
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

const ctaPrimary = cn(
  buttonVariants({ variant: "outline" }),
  "h-auto border-primary px-5 py-2.5 text-sm text-primary hover:bg-primary hover:text-primary-foreground",
);
const ctaSecondary = cn(
  buttonVariants({ variant: "outline" }),
  "h-auto px-5 py-2.5 text-sm",
);

function Wordmark({ lead }: { lead?: boolean }) {
  return (
    <span className={cn("font-label", lead && "text-foreground")}>chat-rs</span>
  );
}

function InstallPill() {
  const [copied, setCopied] = useState(false);
  const copy = () => {
    if (typeof navigator === "undefined" || !navigator.clipboard) return;
    navigator.clipboard
      .writeText("cargo add chat-rs")
      .then(() => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      })
      .catch(() => {});
  };
  return (
    <button
      type="button"
      onClick={copy}
      aria-label="Copy install command"
      className="group/install inline-flex items-center gap-2.5 border border-border bg-card px-4 py-2.5 text-left"
    >
      <span className="font-label text-xs text-primary">$</span>
      <code className="text-sm">cargo add chat-rs</code>
      {copied ? (
        <Check className="size-3.5 text-primary" />
      ) : (
        <Copy className="size-3.5 text-muted-foreground opacity-60 transition-opacity group-hover/install:opacity-100" />
      )}
    </button>
  );
}

function Eyebrow({ children }: { children: React.ReactNode }) {
  return (
    <p className="font-label text-xs uppercase tracking-[0.2em] text-muted-foreground">
      {children}
    </p>
  );
}

function SectionBar({ index, title }: { index: string; title: string }) {
  return (
    <div className="border-b border-border px-4 py-4 sm:px-6 sm:py-5">
      <h2 className="font-label flex items-baseline gap-3 text-lg sm:text-2xl">
        <span className="text-primary">{index}</span>
        <span className="tracking-tight text-foreground">{title}</span>
      </h2>
    </div>
  );
}

function FeatureCard({ feature }: { feature: (typeof FEATURES)[number] }) {
  return (
    <Card className="gap-3 border-0 bg-card p-6 ring-0 sm:p-8">
      <div className="flex items-center gap-3">
        <span className="font-label text-xs uppercase tracking-[0.2em] text-muted-foreground">
          {feature.label}
        </span>
        {feature.tag && (
          <Badge
            variant="outline"
            className="border-primary text-[10px] uppercase tracking-[0.15em] text-primary"
          >
            {feature.tag}
          </Badge>
        )}
      </div>
      <h3 className="text-base font-semibold tracking-tight text-foreground">
        {feature.title}
      </h3>
      <p className="text-sm leading-relaxed text-muted-foreground">
        {feature.body}
      </p>
    </Card>
  );
}

function Home() {
  return (
    <HomeLayout {...baseOptions()}>
      <main className="mx-auto flex w-full max-w-5xl flex-1 flex-col border-x border-border">
        {/* Hero */}
        <section className="relative overflow-hidden">
          <div className="pointer-events-none absolute inset-0" aria-hidden>
            <FaultyTerminal
              className="h-full w-full"
              scale={1.6}
              gridMul={[2, 1]}
              digitSize={1.3}
              timeScale={0.4}
              scanlineIntensity={0.5}
              glitchAmount={1}
              flickerAmount={0.6}
              noiseAmp={1}
              chromaticAberration={0}
              curvature={0}
              tint="#E97C3C"
              mouseReact={false}
              pageLoadAnimation
              brightness={0.9}
              dpr={1}
            />
          </div>
          {/* Left-to-right wash: solid page color on the left for legible copy,
              clearing to transparent by ~40% so the animation shows on the
              right. A faint bottom fade blends into the next section. */}
          <div
            className="pointer-events-none absolute inset-0 bg-gradient-to-r from-background from-0% to-transparent to-[42%]"
            aria-hidden
          />
          <div
            className="pointer-events-none absolute inset-x-0 bottom-0 h-24 bg-gradient-to-b from-transparent to-background"
            aria-hidden
          />
          <div className="relative flex max-w-xl flex-col gap-7 px-4 py-24 sm:px-6 sm:py-36">
            <Eyebrow>A Creology runtime · Rust</Eyebrow>
            <h1 className="max-w-3xl text-balance text-3xl font-medium leading-[1.1] tracking-tight text-foreground sm:text-5xl">
              One Rust API for every model.
            </h1>
            <p className="max-w-2xl text-balance text-base leading-relaxed text-muted-foreground">
              <Wordmark lead /> is the interaction layer between your app and any
              language model. Streaming, multimodal content, tools, structured
              output, and multi-provider routing, behind one type-safe API
              across OpenAI, Claude, Gemini, Ollama, and more.
            </p>
            <div className="flex flex-wrap items-center gap-3 pt-2">
              <Link
                to="/docs/$"
                params={{ _splat: "getting-started" }}
                className={ctaPrimary}
              >
                Get started
              </Link>
              <a href="https://github.com/EggerMarc/chat-rs" className={ctaSecondary}>
                View on GitHub
              </a>
              <InstallPill />
            </div>
          </div>
        </section>

        {/* Code gallery */}
        <section className="border-t border-border px-4 py-12 sm:px-6 sm:py-16">
          <CodeGallery />
        </section>

        {/* 01 - Features */}
        <section className="border-t border-border">
          <SectionBar index="01" title="Everything you need to talk to models" />
          <p className="max-w-2xl px-4 pb-14 pt-12 text-sm leading-relaxed text-muted-foreground sm:px-6">
            From a one-line completion to streaming, tools, and typed output,{" "}
            <Wordmark /> hands you the whole interaction surface and keeps it
            identical across every provider you reach for.
          </p>
          <div className="px-4 pb-20 sm:px-6">
            <div className="grid grid-cols-1 gap-px border border-border bg-border sm:grid-cols-2 lg:grid-cols-3">
              {FEATURES.map((f) => (
                <FeatureCard key={f.label} feature={f} />
              ))}
            </div>
          </div>
        </section>

        {/* 02 - How the runtime works */}
        <section className="border-t border-border">
          <SectionBar index="02" title="How the runtime works" />
          <div className="grid grid-cols-1 lg:grid-cols-2">
            <div className="flex flex-col gap-8 px-4 py-12 sm:px-6 sm:py-16">
              <p className="max-w-md text-sm leading-relaxed text-muted-foreground">
                Under the hood, <Wordmark /> runs the reason-and-act loop for
                you: it calls the model, runs the tools it asks for, feeds the
                results back, and repeats until the model is done. You write a
                provider; the loop takes care of the rest.
              </p>
              <ul className="flex flex-col gap-6">
                {RUNTIME_POINTS.map((p, i) => (
                  <li key={p.title} className="flex gap-4">
                    <span className="font-label text-xs text-primary">
                      {String(i + 1).padStart(2, "0")}
                    </span>
                    <div>
                      <h3 className="mb-1 text-sm font-semibold tracking-tight text-foreground">
                        {p.title}
                      </h3>
                      <p className="max-w-sm text-sm leading-relaxed text-muted-foreground">
                        {p.body}
                      </p>
                    </div>
                  </li>
                ))}
              </ul>
            </div>
            <div className="border-t border-border lg:border-l lg:border-t-0">
              <div className="border-b border-border px-4 py-2.5 sm:px-6">
                <span className="font-label text-xs uppercase tracking-[0.2em] text-primary">
                  provider.rs
                </span>
              </div>
              <CopyableCode code={RUNTIME_SNIPPET}>
                <DynamicCodeBlock
                  lang="rust"
                  code={RUNTIME_SNIPPET}
                  codeblock={{
                    allowCopy: false,
                    className: "!my-0 !rounded-none !border-0",
                  }}
                />
              </CopyableCode>
            </div>
          </div>
        </section>

        {/* 03 - Small on purpose */}
        <section className="border-t border-border">
          <SectionBar index="03" title="Small on purpose" />
          <p className="max-w-2xl px-4 pb-14 pt-12 text-sm leading-relaxed text-muted-foreground sm:px-6">
            <Wordmark /> is the runtime, not the framework. You get a rock-solid
            interaction layer; agents, workflows, and memory stay yours to build
            on top, however you like, with whatever architecture fits.
          </p>
          <div className="grid grid-cols-1 gap-4 px-4 pb-16 sm:gap-6 sm:px-6 md:grid-cols-2">
            <Card className="gap-0 border border-border p-6 ring-0 sm:p-10">
              <Eyebrow>In the box</Eyebrow>
              <ul className="mt-6 space-y-3">
                {IN_BOX.map((item) => (
                  <li key={item} className="flex items-baseline gap-3 text-sm">
                    <span className="font-label text-xs text-primary">+</span>
                    <span className="text-foreground">{item}</span>
                  </li>
                ))}
              </ul>
            </Card>
            <Card className="gap-0 border border-border p-6 ring-0 sm:p-10">
              <Eyebrow>Bring your own</Eyebrow>
              <ul className="mt-6 space-y-3">
                {BRING_YOUR_OWN.map((item) => (
                  <li key={item} className="flex items-baseline gap-3 text-sm">
                    <span className="font-label text-xs text-muted-foreground">
                      ·
                    </span>
                    <span className="text-muted-foreground">{item}</span>
                  </li>
                ))}
              </ul>
            </Card>
          </div>
        </section>

        {/* CTA */}
        <section className="border-t border-border px-4 py-20 sm:px-6 sm:py-28">
          <Eyebrow>Start here</Eyebrow>
          <h2 className="mt-5 max-w-2xl text-balance text-2xl font-medium tracking-tight text-foreground sm:text-3xl">
            Ship your first completion in five minutes.
          </h2>
          <p className="mt-5 max-w-xl text-sm leading-relaxed text-muted-foreground">
            Add the crate, pick a provider, and call <code>complete</code>. Swap
            in streaming, tools, or routing whenever you're ready. The API stays
            the same.
          </p>
          <div className="mt-7 flex flex-wrap items-center gap-3">
            <Link
              to="/docs/$"
              params={{ _splat: "getting-started" }}
              className={ctaPrimary}
            >
              Read the docs
            </Link>
            <a href="https://github.com/EggerMarc/chat-rs" className={ctaSecondary}>
              View on GitHub
            </a>
          </div>
        </section>

        {/* Colophon */}
        <footer className="flex flex-col gap-4 border-t border-border px-4 py-12 sm:flex-row sm:items-center sm:justify-between sm:px-6 sm:py-16">
          <span className="font-label text-xs uppercase tracking-[0.2em] text-muted-foreground">
            Open source · MIT
          </span>
          <a
            href="https://creology.co"
            className="text-xs text-muted-foreground/70 transition-colors hover:text-muted-foreground"
          >
            Built by Creology
          </a>
        </footer>
      </main>
    </HomeLayout>
  );
}
