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
- **Flashcard review** — click to flip, shuffle, and browse by document.
- **Persistence** — SQLite database + on-disk file storage. Reload the app
  and your documents and flashcards are all restored.

## Project structure

```
auto-flashcard/
├── api/                 # Rust HTTP backend (Axum)
│   ├── src/             # parser, LLM, DB, routes, chunker
│   ├── .env.example     # template for API keys
│   └── Cargo.toml
├── web/                 # Next.js App Router frontend
│   ├── src/app/         # pages
│   ├── src/components/  # UI components (shadcn/ui)
│   ├── src/lib/         # API client, types, utils
│   └── package.json
└── docs/                # Architecture diagrams & plan
    ├── architecture.html
    ├── architecture.png
    └── architecture.md
```

## Tech stack

- **Frontend**: Next.js 16 (App Router), Tailwind CSS v4, TypeScript, shadcn/ui.
- **Backend**: Rust + Axum + SQLite (PostgreSQL/S3 later).
- **Parsing**: Rust PDF/Markdown extraction ported from the native app.
  PowerPoint support is planned.
- **LLM**: Multi-provider chain — Groq, Cerebras, OpenAI (all server-side,
  keys never sent to the browser).
- **Progress**: polling (fits Vercel/serverless). Queue added later.

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
cd web
npm install
npm run dev
```

Runs on `http://localhost:3000` by default.

The frontend expects the API at `http://localhost:3001`. For production, set
`NEXT_PUBLIC_API_URL` to your deployed backend URL.

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

## Development notes

- `api/data/` holds the SQLite DB and uploaded files in dev. For production,
  use a persistent volume (Render/Railway/Fly) or migrate to Postgres + S3.
- LLM keys live only in `api/.env` (gitignored). Never commit secrets.
- See `docs/architecture.md` for the full architecture plan and data model.
- Tests: `cd api && cargo test` (33 tests).

## Roadmap

- PowerPoint (.pptx) parsing
- Flashcard editing, deletion, and manual creation
- Study/quiz mode with self-scoring and spaced repetition
- Export to Anki (.apkg), CSV, JSON, printable PDF
- Page/section selection for targeted generation
- Auth for Monica (simple password or magic-link cookie)
- Deployment: frontend → Vercel, backend → Render/Railway/Fly
- Message queue (Redis/SQS) for large-document processing
