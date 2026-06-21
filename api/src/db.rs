use std::path::{Path, PathBuf};

use anyhow::Context;
use rusqlite::{params, Connection};
use sha2::Digest;


use crate::models::{Chunk, DocumentInfo, Flashcard, PageContent};

pub struct AppState {
    pub db_path: PathBuf,
    pub uploads_dir: PathBuf,
}

impl AppState {
    pub fn new(db_path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let uploads_dir = db_path
            .parent()
            .map(|p| p.join("uploads"))
            .unwrap_or_else(|| PathBuf::from("uploads"));

        std::fs::create_dir_all(&db_path.parent().unwrap_or(Path::new(".")))
            .context("create data dir")?;
        std::fs::create_dir_all(&uploads_dir).context("create uploads dir")?;

        let conn = open_path(&db_path)?;
        init_schema(&conn)?;
        migrate(&conn)?;

        Ok(Self {
            db_path,
            uploads_dir,
        })
    }

    pub fn open(&self) -> anyhow::Result<Connection> {
        open_path(&self.db_path)
    }
}

fn open_path(path: &Path) -> anyhow::Result<Connection> {
    Connection::open(path).context("open sqlite")
}

pub fn init_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS documents (
            id TEXT PRIMARY KEY,
            filename TEXT NOT NULL,
            file_type TEXT NOT NULL,
            page_count INTEGER NOT NULL DEFAULT 0,
            total_chars INTEGER NOT NULL DEFAULT 0,
            file_hash TEXT NOT NULL UNIQUE,
            storage_key TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS files (
            id TEXT PRIMARY KEY,
            document_id TEXT NOT NULL REFERENCES documents(id),
            storage_key TEXT NOT NULL,
            content_hash TEXT NOT NULL,
            size_bytes INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS pages (
            id TEXT PRIMARY KEY,
            document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
            page_num INTEGER NOT NULL,
            text TEXT NOT NULL,
            char_offset INTEGER NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_pages_document ON pages(document_id);

        CREATE TABLE IF NOT EXISTS chunks (
            id TEXT PRIMARY KEY,
            document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
            content_hash TEXT NOT NULL,
            content TEXT NOT NULL,
            token_count INTEGER NOT NULL DEFAULT 0,
            start_page INTEGER NOT NULL,
            end_page INTEGER NOT NULL,
            start_char INTEGER NOT NULL DEFAULT 0,
            end_char INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_chunks_document ON chunks(document_id);

        CREATE TABLE IF NOT EXISTS generation_jobs (
            id TEXT PRIMARY KEY,
            document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
            status TEXT NOT NULL,
            progress INTEGER NOT NULL DEFAULT 0,
            total INTEGER NOT NULL DEFAULT 0,
            error_message TEXT,
            density TEXT,
            use_llm INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_jobs_document ON generation_jobs(document_id);

        CREATE TABLE IF NOT EXISTS flashcards (
            id TEXT PRIMARY KEY,
            document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
            chunk_id TEXT REFERENCES chunks(id),
            question TEXT NOT NULL,
            answer TEXT NOT NULL,
            card_type TEXT NOT NULL,
            source_ref TEXT NOT NULL,
            tags TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_flashcards_document ON flashcards(document_id);
        "#,
    )
    .context("init schema")?;

    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct StoredDocument {
    pub id: String,
    pub filename: String,
    pub file_type: String,
    pub page_count: u32,
    pub total_chars: usize,
    pub file_hash: String,
    pub storage_key: String,
    pub status: String,
    pub created_at: String,
}

pub fn find_document_by_hash(conn: &Connection, hash: &str) -> anyhow::Result<Option<StoredDocument>> {
    let mut stmt = conn.prepare(
        "SELECT id, filename, file_type, page_count, total_chars, file_hash, storage_key, status, created_at
         FROM documents WHERE file_hash = ?1",
    )?;
    let mut rows = stmt.query(params![hash])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_document(row)?))
    } else {
        Ok(None)
    }
}

pub fn find_document_by_id(conn: &Connection, id: &str) -> anyhow::Result<Option<StoredDocument>> {
    let mut stmt = conn.prepare(
        "SELECT id, filename, file_type, page_count, total_chars, file_hash, storage_key, status, created_at
         FROM documents WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(map_document(row)?))
    } else {
        Ok(None)
    }
}

fn map_document(row: &rusqlite::Row) -> rusqlite::Result<StoredDocument> {
    Ok(StoredDocument {
        id: row.get(0)?,
        filename: row.get(1)?,
        file_type: row.get(2)?,
        page_count: row.get(3)?,
        total_chars: row.get(4)?,
        file_hash: row.get(5)?,
        storage_key: row.get(6)?,
        status: row.get(7)?,
        created_at: row.get(8)?,
    })
}

pub fn list_documents(conn: &Connection) -> anyhow::Result<Vec<StoredDocument>> {
    let mut stmt = conn.prepare(
        "SELECT id, filename, file_type, page_count, total_chars, file_hash, storage_key, status, created_at
         FROM documents ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |row| map_document(row))?;
    rows.collect::<Result<Vec<_>, _>>()
        .context("collect documents")
}

pub fn get_pages(conn: &Connection, document_id: &str) -> anyhow::Result<Vec<PageContent>> {
    let mut stmt = conn.prepare(
        "SELECT page_num, text, char_offset FROM pages WHERE document_id = ?1 ORDER BY page_num",
    )?;
    let rows = stmt.query_map(params![document_id], |row| {
        Ok(PageContent {
            page_num: row.get(0)?,
            text: row.get(1)?,
            char_offset: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .context("collect pages")
}

pub fn insert_document(
    conn: &mut Connection,
    doc: &StoredDocument,
    size_bytes: usize,
    pages: &[PageContent],
) -> anyhow::Result<()> {
    let tx = conn.transaction()?;

    tx.execute(
        "INSERT INTO documents (id, filename, file_type, page_count, total_chars, file_hash, storage_key, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            doc.id,
            doc.filename,
            doc.file_type,
            doc.page_count,
            doc.total_chars,
            doc.file_hash,
            doc.storage_key,
            doc.status,
            doc.created_at,
        ],
    )
    .context("insert document")?;

    let file_id = uuid::Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO files (id, document_id, storage_key, content_hash, size_bytes)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![file_id, doc.id, doc.storage_key, doc.file_hash, size_bytes as i64],
    )
    .context("insert file")?;

    for page in pages {
        let page_id = uuid::Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO pages (id, document_id, page_num, text, char_offset)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                page_id,
                doc.id,
                page.page_num,
                page.text,
                page.char_offset as i64,
            ],
        )
        .context("insert page")?;
    }

    tx.commit()?;
    Ok(())
}

pub fn to_document_info(doc: &StoredDocument, pages: Vec<PageContent>) -> DocumentInfo {
    DocumentInfo {
        id: doc.id.clone(),
        filename: doc.filename.clone(),
        file_type: doc.file_type.clone(),
        page_count: doc.page_count,
        total_chars: doc.total_chars,
        pages,
    }
}

fn has_column(conn: &Connection, table: &str, column: &str) -> anyhow::Result<bool> {
    let mut stmt = conn.prepare("SELECT name FROM pragma_table_info(?1) WHERE name = ?2")?;
    let mut rows = stmt.query(params![table, column])?;
    Ok(rows.next()?.is_some())
}

fn migrate(conn: &Connection) -> anyhow::Result<()> {
    if !has_column(conn, "chunks", "start_char")? {
        conn.execute(
            "ALTER TABLE chunks ADD COLUMN start_char INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    if !has_column(conn, "chunks", "end_char")? {
        conn.execute(
            "ALTER TABLE chunks ADD COLUMN end_char INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Chunks
// ---------------------------------------------------------------------------

pub fn insert_chunks(conn: &mut Connection, chunks: &[Chunk]) -> anyhow::Result<()> {
    let tx = conn.transaction()?;
    for chunk in chunks {
        let content_hash = hex::encode(sha2::Sha256::digest(chunk.content.as_bytes()));
        tx.execute(
            "INSERT INTO chunks (id, document_id, content_hash, content, token_count, start_page, end_page, start_char, end_char)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                chunk.id,
                chunk.document_id,
                content_hash,
                chunk.content,
                chunk.token_count as i64,
                chunk.start_page,
                chunk.end_page,
                chunk.start_char as i64,
                chunk.end_char as i64,
            ],
        )
        .context("insert chunk")?;
    }
    tx.commit()?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Flashcards
// ---------------------------------------------------------------------------

pub fn insert_flashcards(conn: &mut Connection, cards: &[Flashcard]) -> anyhow::Result<()> {
    let tx = conn.transaction()?;
    for card in cards {
        let source_ref = serde_json::to_string(&card.source_ref).context("serialize source_ref")?;
        let tags = serde_json::to_string(&card.tags).context("serialize tags")?;
        tx.execute(
            "INSERT INTO flashcards (id, document_id, chunk_id, question, answer, card_type, source_ref, tags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                card.id,
                card.document_id,
                card.chunk_id,
                card.question,
                card.answer,
                card.card_type,
                source_ref,
                tags,
            ],
        )
        .context("insert flashcard")?;
    }
    tx.commit()?;
    Ok(())
}

pub fn get_flashcards(conn: &Connection, document_id: &str) -> anyhow::Result<Vec<Flashcard>> {
    let mut stmt = conn.prepare(
        "SELECT id, document_id, chunk_id, question, answer, card_type, source_ref, tags
         FROM flashcards WHERE document_id = ?1 ORDER BY id"
    )?;
    let rows = stmt.query_map(params![document_id], |row| {
        let source_ref: String = row.get(6)?;
        let tags: String = row.get(7)?;
        Ok(Flashcard {
            id: row.get(0)?,
            document_id: row.get(1)?,
            chunk_id: row.get(2)?,
            question: row.get(3)?,
            answer: row.get(4)?,
            card_type: row.get(5)?,
            source_ref: serde_json::from_str(&source_ref).unwrap_or(crate::models::SourceRef {
                page_start: 0,
                page_end: 0,
                char_start: 0,
                char_end: 0,
                preview: String::new(),
            }),
            tags: serde_json::from_str(&tags).unwrap_or_default(),
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .context("collect flashcards")
}

// ---------------------------------------------------------------------------
// Generation jobs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
pub struct StoredJob {
    pub id: String,
    pub document_id: String,
    pub status: String,
    pub progress: i64,
    pub total: i64,
    pub error_message: Option<String>,
    pub density: Option<String>,
    pub use_llm: bool,
}

pub fn create_job(
    conn: &mut Connection,
    document_id: &str,
    total: usize,
    density: &str,
    use_llm: bool,
) -> anyhow::Result<String> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO generation_jobs (id, document_id, status, progress, total, density, use_llm)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            document_id,
            "queued",
            0,
            total as i64,
            density,
            use_llm as i64,
        ],
    )?;
    Ok(id)
}

pub fn get_job(conn: &Connection, job_id: &str) -> anyhow::Result<Option<StoredJob>> {
    let mut stmt = conn.prepare(
        "SELECT id, document_id, status, progress, total, error_message, density, use_llm
         FROM generation_jobs WHERE id = ?1",
    )?;
    let mut rows = stmt.query(params![job_id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(StoredJob {
            id: row.get(0)?,
            document_id: row.get(1)?,
            status: row.get(2)?,
            progress: row.get(3)?,
            total: row.get(4)?,
            error_message: row.get(5)?,
            density: row.get(6)?,
            use_llm: row.get::<_, i64>(7)? != 0,
        }))
    } else {
        Ok(None)
    }
}

pub fn update_job_progress(conn: &mut Connection, job_id: &str, progress: i64) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE generation_jobs SET progress = ?1, status = ?2 WHERE id = ?3",
        params![progress, "generating", job_id],
    )?;
    Ok(())
}

pub fn set_job_status(
    conn: &mut Connection,
    job_id: &str,
    status: &str,
    error_message: Option<&str>,
) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE generation_jobs SET status = ?1, error_message = ?2 WHERE id = ?3",
        params![status, error_message, job_id],
    )?;
    Ok(())
}

pub fn delete_flashcards_for_document(conn: &mut Connection, document_id: &str) -> anyhow::Result<()> {
    conn.execute(
        "DELETE FROM flashcards WHERE document_id = ?1",
        params![document_id],
    )?;
    Ok(())
}
