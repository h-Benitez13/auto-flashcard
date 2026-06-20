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
