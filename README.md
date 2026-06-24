# auto-flashcard

Web-based flashcard generator for Monica. Upload PDFs or Markdown files,
extract the text, and generate study flashcards with an LLM — then flip,
shuffle, and review. Reload the page and everything is still there.

## Features

- **Upload & parse** — drag-and-drop PDF, Markdown, or text files. Content is
  extracted, paginated, and stored server-side. Duplicate uploads are deduped
  by content hash.
- **Flashcard generation** — LLM-generated Q/A cards with a rule-based
  fallback so you always get cards, even when every API is rate-limited.
  Choose concise / balanced / comprehensive density.
- **Multi-provider LLM chain** — Groq → Cerebras → OpenAI. If one provider
  hits a rate limit, the next takes over automatically. Providers without a
  key are skipped.
- **Document management** — rename documents, soft-delete (move to trash),
  and restore from a collapsible trash section.
- **Flashcard study mode** — focused, centered cards. Click a card to flip
  between question and answer, use Previous/Next buttons, or use keyboard
  shortcuts: `Space` to flip, `←` / `→` to navigate. Shuffle anytime.
- **Answer-side source grounding** — when the answer is revealed, the exact
  source text snippet and page number the LLM used are shown.
- **Card count badges on the home page** — each document with flashcards
  shows a card-count badge and a quick **Review** button.
- **Persistence** — SQLite database + on-disk file storage. Reload the app
  and your documents and flashcards are all restored.

## Project structure

```
auto-flashcard/
├── api/                 # Rust HTTP backend (Axum)
│   ├── src/             # parser, LLM, DB, routes, chunker
│   ├── .env.example     # template for API keys
│   └── Cargo.toml
├── src/                 # Next.js App Router frontend
│   ├── app/             # pages
│   ├── components/      # UI components (shadcn/ui)
│   ├── hooks/           # TanStack Query hooks
│   └── lib/             # API client, types, Zustand store
├── docs/                # Architecture diagrams & plan
│   ├── architecture.html
│   ├── architecture.png
│   └── architecture.md
├── package.json
└── README.md
```

## Tech stack

- **Frontend**: Next.js 16 (App Router), Tailwind CSS v4, TypeScript, shadcn/ui.
- **State management**: TanStack Query v5 (server state) + Zustand (UI state) + Sonner (toasts).
- **Backend**: Rust + Axum + SQLite (PostgreSQL/S3 later).
- **Parsing**: Rust PDF/Markdown/PowerPoint extraction ported from the native app.
  Each PowerPoint slide becomes one page.
- **LLM**: Multi-provider chain — Groq, Cerebras, OpenAI (all server-side,
  keys never sent to the browser).
- **Progress**: adaptive polling with TanStack Query (fits Vercel/serverless). Queue added later.

## Getting started

### 1. Backend

```bash
cd api
cp .env.example .env
# Edit .env and add at least one LLM provider key
cargo run
```

Runs on `http://localhost:3001` by default.

**Environment variables** (in `api/.env`, gitignored):

| Variable           | Required | Description                                      |
| ------------------ | -------- | ------------------------------------------------ |
| `GROQ_API_KEY`     | No*      | Groq API key (free tier, primary provider)       |
| `CEREBRAS_API_KEY` | No*      | Cerebras API key (free tier, first fallback)     |
| `OPENAI_API_KEY`   | No*      | OpenAI API key (paid, safety net)                |
| `DATABASE_PATH`    | No       | SQLite path (default: `data/app.db`)             |
| `PORT`             | No       | Server port (default: `3001`)                    |

\* At least one LLM key enables LLM-generated cards. With no keys, generation
falls back to the built-in rule-based extractor.

**Provider chain order**: Groq → Cerebras → OpenAI. Free providers are tried
first; OpenAI is the paid safety net. When a provider hits a daily rate limit,
the next provider takes over immediately. If all providers fail, cards are
generated with the rule-based fallback so Monica always gets something.

### 2. Frontend

```bash
npm install
npm run dev
```

Runs on `http://localhost:3000` by default.

The frontend expects the API at `http://localhost:3001`. For production, set
`NEXT_PUBLIC_API_URL` to your deployed backend URL.

### Tests

```bash
npm test          # run tests once
npm run test:watch # run tests in watch mode
```

Uses Vitest + React Testing Library. Tests cover:
- Zustand store state changes
- HTTP client error handling
- TanStack Query data fetching hooks
- Mutation optimistic updates and cache invalidation
- Generation job polling behavior
- FlashcardList component (study/grid modes, navigation, flip)

## API endpoints

| Method   | Route                              | Description                        |
| -------- | ---------------------------------- | ---------------------------------- |
| `GET`    | `/health`                          | Health check                       |
| `POST`   | `/upload`                          | Upload & parse a file (multipart)  |
| `GET`    | `/documents`                       | List active documents              |
| `GET`    | `/documents/:id`                   | Get document + extracted pages     |
| `PATCH`  | `/documents/:id`                   | Rename a document                  |
| `DELETE` | `/documents/:id`                   | Soft-delete (move to trash)        |
| `POST`   | `/documents/:id/restore`           | Restore from trash                 |
| `POST`   | `/documents/:id/generate`          | Start flashcard generation job     |
| `GET`    | `/documents/:id/flashcards`        | List flashcards for a document     |
| `GET`    | `/jobs/:id`                        | Poll generation job status         |
| `GET`    | `/trash`                           | List soft-deleted documents        |

## Frontend architecture

### Three-layer state management

1. **TanStack Query** (server state)
   - Documents, flashcards, jobs
   - Handles caching, deduplication, refetching, retries
   - See `src/hooks/useDocuments.ts`, `src/hooks/useMutations.ts`, `src/hooks/useGenerationJob.ts`

2. **Zustand** (UI state)
   - View mode (study/grid), grid columns, trash visibility
   - See `src/lib/store.ts`
   - Lightweight, no Provider needed, similar mental model to Jotai but less granular

3. **React state** (component-level)
   - Local form state, animation state
   - Handled by `useState` in components

### Key frontend patterns

- **Data fetching**: always use a query hook from `src/hooks/useDocuments.ts`
- **Mutations**: always use a mutation hook from `src/hooks/useMutations.ts`
- **Polling**: `useGenerationJobPolling` handles adaptive backoff (1s → 2s → 5s → 10s)
- **Errors**: API errors show Sonner toast notifications via `onError` callbacks
- **Optimistic updates**: rename/delete update the UI immediately, then sync with server

## Development notes

- `api/data/` holds the SQLite DB and uploaded files in dev. For production,
  use a persistent volume (Render/Railway/Fly) or migrate to Postgres + S3.
- LLM keys live only in `api/.env` (gitignored). Never commit secrets.
- See `docs/architecture.md` for the full architecture plan and data model.
- Tests: `cd api && cargo test` (33 tests).

## Deployment

This app is split into two deployable units: the **Next.js frontend** (Vercel)
and the **Rust backend** (Fly.io). They can live on different domains because
the backend already allows cross-origin requests (`CorsLayer::permissive()`).

> **Shared workspace**: everyone with the frontend URL shares the same SQLite
> database and uploaded files on the backend. No per-user isolation yet.

### 1. Backend — Fly.io

The backend needs a **persistent volume** (SQLite DB + uploaded files) and an
**always-on** process because flashcard generation runs in-process with polling.

Prerequisites: [Fly CLI](https://fly.io/docs/hands-on/install-flyctl/) installed
and logged in (`flyctl auth login`).

```bash
cd api

# Launch the app (creates the app on Fly; you can say no to adding a db/postgres)
flyctl launch --name auto-flashcard-api --region lax --no-deploy

# Create a persistent volume for SQLite and uploads (1 GB is plenty to start)
flyctl volumes create auto_flashcard_data --size 1 --region lax --app auto-flashcard-api

# Set the LLM provider secrets. Use fresh keys from each provider dashboard.
flyctl secrets set GROQ_API_KEY=your_groq_key_here \
                   CEREBRAS_API_KEY=your_cerebras_key_here \
                   OPENAI_API_KEY=your_openai_key_here \
                   --app auto-flashcard-api

# Deploy
flyctl deploy
```

When it finishes, Fly gives you a public URL:
```
https://auto-flashcard-api.fly.dev
```

The provided `fly.toml` already sets:
- `PORT=3001`
- `DATABASE_PATH=/data/app.db`
- a mount for the volume at `/data` (so `/data/app.db` and `/data/uploads`
  both persist across redeploys)

### 2. Frontend — Vercel

1. Push this repo to GitHub if you haven't already.
2. In Vercel, **Import Project** → select the repo.
3. Set **Root Directory** to `web`.
4. Add the environment variable:
   ```
   NEXT_PUBLIC_API_URL=https://auto-flashcard-api.fly.dev
   ```
   (`NEXT_PUBLIC_*` is inlined at build time, so you must redeploy after changing it.)
5. Deploy.

Vercel will give you a URL like:
```
https://auto-flashcard-xyz.vercel.app
```

That's the URL you share with Monica.

### 3. Smoke test the live app

1. Open the Vercel URL.
2. Upload a PDF, Markdown, or PowerPoint file.
3. Click **Generate flashcards**.
4. Flip through cards with `Space`, navigate with `←` / `→`.

If uploads fail with a size error, check that the backend logs show
`DefaultBodyLimit::max(100 * 1024 * 1024)` and that Fly is not running behind
a smaller proxy limit.

### 4. Updating the live app

```bash
# Backend changes
cd api && flyctl deploy

# Frontend changes: commit, push, and Vercel auto-deploys.
```

### Security reminders

- Rotate any LLM keys that were ever pasted in chat/log files and use the new
  keys only as Fly secrets.
- Do not commit `.env` files.
- The deployed app has **no authentication yet**; anyone with the URL can upload and
  see all documents. Magic-link auth is on the roadmap (Phase 2). Until then, keep
  the URL private.

## Roadmap

The current release is a single-user flashcard generator. The path forward turns
**auto-flashcard** into a personalized, multi-user study platform: every learner
gets their own documents, decks, and review history, while the core upload →
generate → review loop stays fast and simple.

Each phase below ships independently and is tracked by a GitHub milestone.
Spikes produce decision docs before implementation starts so later phases don't
have to revisit basics.

### ✅ Done
- PowerPoint (.pptx) parsing — each slide becomes one page
- Grid view + provider tracking (Groq → Cerebras → OpenAI chain)
- Soft-delete + trash, document rename, content-hash dedup
- Answer-side source grounding (snippet + page number)
- Adaptive polling for generation jobs

### 🔬 Spikes (design before build)
- [ ] [#4](../../issues/4) — SRS (SM-2) scheduling spec
- [ ] [#5](../../issues/5) — Deck ↔ document model & upload-to-review flow
- [ ] [#6](../../issues/6) — Auth integration plan (email, CORS, CSRF)
- [ ] [#7](../../issues/7) — SQLite → PostgreSQL scaling thresholds

### Phase 0 — Prep
- [ ] [#8](../../issues/8) — `users` table + `documents.user_id` migration + backfill
- [ ] [#9](../../issues/9) — Credentialed CORS + CSRF protection
- [ ] [#10](../../issues/10) — API client 401 handler stub

### Phase 1 — Card lifecycle
- [ ] [#11](../../issues/11) — Card CRUD backend (create, update, soft-delete, tags, flags)
- [ ] [#12](../../issues/12) — CardEditor UI (edit, delete-with-undo, flag, shortcuts)
- [ ] [#13](../../issues/13) — Single-card regenerate (accept/keep-both diff)
- [ ] [#14](../../issues/14) — Card lifecycle tests

### Phase 2 — Auth & per-user isolation
- [ ] [#15](../../issues/15) — Magic-link auth backend
- [ ] [#16](../../issues/16) — Login page + `useUser` hook + 401 redirect
- [ ] [#17](../../issues/17) — User-scope all document/card queries
- [ ] [#18](../../issues/18) — Auth + isolation tests

### Phase 3 — Decks & targeted generation
- [ ] [#19](../../issues/19) — Deck CRUD backend + `deck_cards` junction
- [ ] [#20](../../issues/20) — Decks page + `DeckList` + home integration
- [ ] [#21](../../issues/21) — Page/section selection for generation

### Phase 4 — Spaced repetition
- [ ] [#22](../../issues/22) — SM-2-lite scheduling module
- [ ] [#23](../../issues/23) — `card_reviews` table + due-cards query
- [ ] [#24](../../issues/24) — Review session UI + `ReviewStats`
- [ ] [#25](../../issues/25) — Leech detection + reset deck
- [ ] [#26](../../issues/26) — SRS tests

### Phase 5 — Portability
- [ ] [#27](../../issues/27) — CSV + JSON export
- [ ] [#28](../../issues/28) — Anki `.apkg` export (media + scheduling)
- [ ] [#29](../../issues/29) — CSV + JSON import

### Phase 6 — Shareability & polish
- [ ] [#30](../../issues/30) — Public read-only deck links + fork
- [ ] [#31](../../issues/31) — Per-user LLM spend caps
- [ ] [#32](../../issues/32) — Sentry + `/metrics`
- [ ] [#33](../../issues/33) — PWA + offline review sync
- [ ] [#34](../../issues/34) — Message queue for large documents
- [ ] [#35](../../issues/35) — `card_count` + `due_count` in list responses

See the [open issues board](../../issues) for current priorities.
