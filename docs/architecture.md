# auto-flashcard architecture

## Goals

1. Accept PDF or PowerPoint files, convert them into indexed text, and generate flashcards.
2. Reloading/revisiting the app restores previously uploaded files (no re-upload).
3. Reloading/revisiting the app restores generated flashcards.

## High-level architecture

```mermaid
flowchart LR
    Monica([Monica<br/>Browser])

    subgraph Vercel
        NextApp[Next.js App<br/>App Router]
        NextAPI[Next.js API Routes<br/>Route Handlers]
    end

    subgraph BackendHost
        RustAPI[Rust API Service]
        Worker[Rust Worker]
    end

    Postgres[(PostgreSQL)]
    ObjectStore[(Object Storage<br/>S3 / R2)]
    Queue[(Message Queue<br/>SQS / Redis / BullMQ)]
    LLM[Groq LLM API]

    Monica -->|HTTPS| NextApp
    NextApp -->|REST / Server Actions| NextAPI
    NextAPI -->|proxy + auth| RustAPI

    RustAPI -->|enqueue| Queue
    Worker -->|dequeue| Queue

    RustAPI -->|metadata & jobs| Postgres
    Worker -->|pages / chunks / cards| Postgres

    RustAPI -->|original files| ObjectStore
    Worker -->|read original files| ObjectStore

    Worker -->|generate cards| LLM
```

## Data model

```mermaid
erDiagram
    USER {
        uuid id PK
        string email
        datetime created_at
    }

    DOCUMENT {
        uuid id PK
        uuid user_id FK
        string filename
        string file_type
        int page_count
        int total_chars
        string file_hash
        string storage_key
        string status
        datetime created_at
    }

    FILE {
        uuid id PK
        uuid document_id FK
        string storage_key
        string content_hash
        int size_bytes
    }

    PAGE {
        uuid id PK
        uuid document_id FK
        int page_num
        text text
        int char_offset
    }

    CHUNK {
        uuid id PK
        uuid document_id FK
        string content_hash
        text content
        int token_count
        int start_page
        int end_page
    }

    GENERATION_JOB {
        uuid id PK
        uuid document_id FK
        string status
        int progress
        int total
        string error_message
        string density
        boolean use_llm
    }

    FLASHCARD {
        uuid id PK
        uuid document_id FK
        uuid chunk_id FK
        string question
        text answer
        string card_type
        json source_ref
        string[] tags
    }

    USER ||--o{ DOCUMENT : owns
    DOCUMENT ||--|| FILE : has
    DOCUMENT ||--o{ PAGE : contains
    DOCUMENT ||--o{ CHUNK : contains
    DOCUMENT ||--o{ GENERATION_JOB : tracks
    CHUNK ||--o{ FLASHCARD : generates
```

## Upload → generate sequence

```mermaid
sequenceDiagram
    participant M as Monica Browser
    participant N as Next.js API
    participant R as Rust API
    participant W as Rust Worker
    participant DB as PostgreSQL
    participant G as Groq

    M->>N: POST /api/upload (multipart file)
    N->>R: forward file + user context
    R->>R: compute content hash
    R->>R: store original file to Object Storage
    R->>DB: insert Document + File records
    R->>W: enqueue parse_job (via queue)
    R-->>N: document_id, job_id
    N-->>M: upload accepted

    Note over M,N: Poll every 1-2s
    loop Job polling
        M->>N: GET /api/jobs/:id
        N->>R: GET /jobs/:id
        R->>DB: read job status
        R-->>N: status, progress, total
        N-->>M: render progress
    end

    W->>W: dequeue parse_job
    W->>W: download file from Object Storage
    W->>W: extract pages (PDF / PPTX)
    W->>DB: insert Pages
    W->>DB: set job status = parsed

    M->>N: POST /api/documents/:id/generate (pages + density)
    N->>R: forward request
    R->>DB: read selected Pages
    R->>R: build Chunks
    R->>DB: insert Chunks
    R->>W: enqueue generate_job per chunk
    R-->>N: job_id
    N-->>M: generation started

    Note over M,N: Poll every 1-2s
    loop Job polling
        M->>N: GET /api/jobs/:id
        N-->>M: progress
    end

    W->>W: dequeue generate_job
    W->>G: call Groq with chunk + density
    G-->>W: JSON cards
    W->>W: grounding check
    W->>DB: insert Flashcards
    W->>DB: update job progress
    Note over M: Cards appear on completion
```

## Implementation plan

1. **Scaffold projects**
   - `api/` — Rust Axum service.
   - `web/` — Next.js App Router.

2. **Port existing Rust logic**
   - Reuse PDF/Markdown parsers from the native app.
   - Add HTTP routes: `POST /upload`, `GET /documents`, `GET /documents/:id`, `POST /documents/:id/generate`, `GET /jobs/:id`.
   - Keep LLM generation logic on the server; read `GROQ_API_KEY` from `.env`.

3. **Persistence (MVP: SQLite + filesystem)**
   - `Document`, `File`, `Page`, `Chunk`, `GenerationJob`, `Flashcard` tables.
   - Original files stored on disk in `api/data/uploads`.
   - Content-hash uploads to skip duplicate parsing.

4. **Frontend**
   - File upload page with drag-and-drop.
   - Document list + detail view showing extracted pages.
   - Page/density selection and generate button.
   - Flashcard list with flip/shuffle.
   - Polling for job progress.

5. **Auth**
   - Start simple (single-user password cookie for Monica).
   - Leave schema/user table in place for proper auth later.

6. **Later improvements**
   - PostgreSQL + S3/R2 object storage.
   - Message queue (Redis/BullMQ or SQS) for parse/generate jobs.
   - PowerPoint parsing.
   - Real-time progress via SSE if needed (not WebSockets, due to Vercel).

## Key decisions

- **Rust stays the backend**: We keep the existing parser/LLM code as a service rather than rewriting it in TypeScript.
- **Vercel for the frontend**: Easy to share with Monica.
- **Polling for progress**: WebSockets are not viable on Vercel; SSE is possible but overkill for single-user progress bars.
- **Separate repos**: `auto-flashcard` (web) and `flashcards` (native SvelteKit/Tauri) remain independent.

## Files

- `docs/architecture.html` — interactive Mermaid diagrams.
- `docs/architecture.png` — rendered screenshot.

---

## Evolution log

### 2026-06-24 — From generator to personalized study platform

This entry records the architecture decisions made after reviewing the current
state of the repo and deciding what to build next. It does not replace the
original architecture above; it extends it.

#### Decision

Turn **auto-flashcard** from a single-user flashcard generator into a
multi-user, personalized study platform. The minimum viable transformation is
the combination of **identity + spaced repetition + card lifecycle + decks**.
Everything else (sharing, export/import, observability, PWA) builds on those
primitives.

Card lifecycle is intentionally scheduled before auth (Phase 1 vs Phase 2) so
Monica gets edit/delete/regenerate capabilities immediately without waiting for
the full auth integration.

#### Adjusted component map

| New / modified module | Responsibility |
| --------------------- | -------------- |
| `api/src/auth.rs` | Magic-link tokens, session cookies, `require_user` extractor |
| `api/src/db.rs` | `users`, `decks`, `deck_cards`, `card_reviews`, `card_flags`; user-scoped queries |
| `api/src/srs.rs` | Pure SM-2-lite scheduler |
| `api/src/export.rs` | CSV, JSON, and Anki `.apkg` serialization |
| `api/src/import.rs` | CSV, JSON, and Anki `.apkg` ingestion |
| `api/src/spend.rs` | Per-user LLM usage tracking and soft cap |
| `api/src/metrics.rs` | `/metrics` endpoint for operational signals |
| `src/lib/auth.ts` | `useUser()` hook, session redirect logic |
| `src/app/login/page.tsx` | Magic-link email form |
| `src/app/review/page.tsx` | SRS daily review session |
| `src/app/decks/page.tsx` | Deck library |
| `src/components/CardEditor.tsx` | Inline card edit, delete-with-undo, flag |
| `src/components/DeckList.tsx` | Deck list with card/due counts |
| `src/components/ReviewStats.tsx` | Session summary + streak widget |
| `src/components/ExportMenu.tsx` | Export format selector |
| `src/hooks/useCardMutations.ts` | Card CRUD + optimistic updates |
| `src/hooks/useDecks.ts` | Deck queries/mutations |
| `src/hooks/useReview.ts` | Due-queue and review mutation |

#### Data-flow additions

1. **Auth**: `POST /auth/magic-link` → email → `GET /auth/callback` sets
   `HttpOnly` `sid` cookie → `require_user` extractor scopes every route.
2. **Review**: `GET /review/due?deck=:id` returns due cards → user flips and
   rates → `POST /cards/:id/review` → `srs.rs` computes next `due_at`.
3. **Card lifecycle**: `PATCH/DELETE /cards/:id` with soft-delete; deleted cards
   are excluded from due queries and deck stats.
4. **Decks**: `deck_cards` junction allows cards from multiple documents to be
   grouped into a study set.
5. **Export / import**: `GET /decks/:id/export?format=...` streams files;
   `POST /decks/:id/import` ingests new cards with preview.

#### New risks and mitigations

| Risk | Mitigation |
| ---- | ---------- |
| Magic-link auth depends on an email sender | Choose Resend/SES with a fallback; test with Mailhog/stdout in dev |
| Cross-origin cookies are fragile | Use `SameSite=None; Secure` in prod, relaxed dev config, and CSRF tokens |
| SRS defaults may feel wrong | Produce `docs/srs-spec.md` in a spike; ship a "reset deck" escape hatch |
| Per-user LLM spend tracking is approximate per provider | Track best-effort tokens and warn/fallback rather than hard-cut mid-job |
| SQLite won't scale past ~10 concurrent writers | Define load-test thresholds in `docs/scaling-plan.md`; migrate to PostgreSQL when crossed |
| Anki `.apkg` export is fiddly | Ship CSV/JSON first; `.apkg` as a follow-up |

#### Decisions that need input (tracked as spikes)

- SRS scheduling defaults: #4
- Deck/document relationship and deletion semantics: #5
- Email provider + cookie/CSRF strategy: #6
- SQLite → PostgreSQL thresholds: #7

#### Roadmap snapshot

The canonical roadmap lives in `README.md`. Issues are tracked in GitHub under
milestones `Spikes`, `Phase 0 — Prep`, `Phase 1 — Card lifecycle`,
`Phase 2 — Auth & per-user isolation`, `Phase 3 — Decks & targeted generation`,
`Phase 4 — SRS`, `Phase 5 — Portability`, and `Phase 6 — Shareability & polish`.
