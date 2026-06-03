# chat-rs docs

Documentation site and landing page for [chat-rs](https://github.com/EggerMarc/chat-rs).

**Stack:** [TanStack Start](https://tanstack.com/start) (Vite, SSR) ·
[Fumadocs](https://fumadocs.dev) + MDX · deployed to **Cloudflare Workers** with
[Alchemy](https://alchemy.run).

## Structure

```
docs/
├── content/docs/        ← MDX documentation pages
├── src/
│   ├── routes/
│   │   ├── index.tsx     ← landing page
│   │   ├── docs/$.tsx    ← Fumadocs catch-all docs route
│   │   └── api/search.ts ← search endpoint
│   ├── lib/source.ts     ← Fumadocs content source
│   └── components/mdx.tsx
├── source.config.ts      ← fumadocs-mdx collection config
├── alchemy.run.ts        ← Cloudflare deploy definition
└── vite.config.ts
```

## Develop

```sh
bun install
bun run dev        # alchemy dev — Vite + local Cloudflare (miniflare), no account needed
```

Open http://localhost:5173. Editing files in `content/docs/*.mdx` hot-reloads.

## Deploy to Cloudflare

Deploys run through Alchemy, which needs Cloudflare credentials. Authenticate
once with either:

```sh
bunx wrangler login            # interactive OAuth, or…
export CLOUDFLARE_API_TOKEN=…  # a scoped API token for CI
```

Set an `ALCHEMY_PASSWORD` (used to encrypt local Alchemy state) in `.env`:

```sh
cp .env.example .env   # then edit ALCHEMY_PASSWORD
```

Then:

```sh
bun run deploy     # alchemy deploy — builds and pushes the Worker
bun run destroy    # tears the stack back down
```

`deploy` prints the deployed `*.workers.dev` URL.
