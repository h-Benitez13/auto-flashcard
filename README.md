# auto-flashcard

Web-based flashcard generator for Monica. Upload PDFs or PowerPoints, extract the text, and generate flashcards with an LLM.

This is the new web codebase. The original SvelteKit + Tauri native app remains in `Creativity/flashcards` and is not touched.

## Project structure

```
auto-flashcard/
├── api/                 # Rust HTTP backend (Axum)
│   ├── src/             # parser, LLM, DB, routes
│   └── Cargo.toml
├── web/                 # Next.js App Router frontend
│   ├── src/app/         # pages/components
│   └── package.json
└── docs/                # Architecture diagrams & plan
    ├── architecture.html
    ├── architecture.png
    └── architecture.md
```

## Tech stack

- **Frontend**: Next.js 16 (App Router), Tailwind CSS, TypeScript.
- **Backend**: Rust + Axum + SQLite (PostgreSQL/S3 later).
- **Parsing**: Existing Rust PDF/Markdown extraction logic ported from the native app. PowerPoint support is next.
- **LLM**: Groq (server-side only, key never sent to the browser).
- **Progress**: polling for now (fits Vercel/serverless). Queue added later.

## Getting started

1. **Backend**
   ```bash
   cd api
   echo "GROQ_API_KEY=your_key" > .env
   cargo run
   ```
   Runs on `http://localhost:3001` by default.

2. **Frontend**
   ```bash
   cd web
   npm install
   npm run dev
   ```
   Runs on `http://localhost:3000` by default.

## Development notes

- `api/data/` is used for local SQLite DB and uploaded files in dev.
- The Groq API key is only present in `api/.env`. Rotate any previously build-baked keys.
- See `docs/architecture.md` for the full plan.
