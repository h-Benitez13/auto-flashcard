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
    let conn = Connection::open(path).context("open sqlite")?;
    conn.execute_batch("PRAGMA foreign_keys = ON")?;
    Ok(conn)
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
            status_message TEXT,
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
            tags TEXT NOT NULL,
            provider TEXT NOT NULL DEFAULT 'llm'
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
    pub deleted_at: Option<String>,
}

/// Look up a document by hash regardless of soft-delete state.
pub fn find_document_by_hash_including_deleted(conn: &Connection, hash: &str) -> anyhow::Result<Option<StoredDocument>> {
    let mut stmt = conn.prepare(
        "SELECT id, filename, file_type, page_count, total_chars, file_hash, storage_key, status, created_at, deleted_at
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
        "SELECT id, filename, file_type, page_count, total_chars, file_hash, storage_key, status, created_at, deleted_at
         FROM documents WHERE id = ?1 AND deleted_at IS NULL",
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
        deleted_at: row.get(9)?,
    })
}

pub fn list_documents(conn: &Connection) -> anyhow::Result<Vec<StoredDocument>> {
    let mut stmt = conn.prepare(
        "SELECT id, filename, file_type, page_count, total_chars, file_hash, storage_key, status, created_at, deleted_at
         FROM documents WHERE deleted_at IS NULL ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |row| map_document(row))?;
    rows.collect::<Result<Vec<_>, _>>()
        .context("collect documents")
}

pub fn list_trash_documents(conn: &Connection) -> anyhow::Result<Vec<StoredDocument>> {
    let mut stmt = conn.prepare(
        "SELECT id, filename, file_type, page_count, total_chars, file_hash, storage_key, status, created_at, deleted_at
         FROM documents WHERE deleted_at IS NOT NULL ORDER BY deleted_at DESC",
    )?;
    let rows = stmt.query_map([], |row| map_document(row))?;
    rows.collect::<Result<Vec<_>, _>>()
        .context("collect trashed documents")
}

/// Returns true if a row was updated.
pub fn rename_document(conn: &mut Connection, id: &str, new_filename: &str) -> anyhow::Result<bool> {
    let count = conn.execute(
        "UPDATE documents SET filename = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![new_filename, id],
    )?;
    Ok(count > 0)
}

/// Returns true if a row was soft-deleted.
pub fn soft_delete_document(conn: &mut Connection, id: &str) -> anyhow::Result<bool> {
    let now = chrono::Utc::now().to_rfc3339();
    let count = conn.execute(
        "UPDATE documents SET deleted_at = ?1 WHERE id = ?2 AND deleted_at IS NULL",
        params![now, id],
    )?;
    Ok(count > 0)
}

/// Returns true if a row was restored.
pub fn restore_document(conn: &mut Connection, id: &str) -> anyhow::Result<bool> {
    let count = conn.execute(
        "UPDATE documents SET deleted_at = NULL WHERE id = ?1",
        params![id],
    )?;
    Ok(count > 0)
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
    if !has_column(conn, "documents", "deleted_at")? {
        conn.execute("ALTER TABLE documents ADD COLUMN deleted_at TEXT", [])?;
    }
    if !has_column(conn, "flashcards", "provider")? {
        conn.execute("ALTER TABLE flashcards ADD COLUMN provider TEXT NOT NULL DEFAULT 'llm'", [])?;
    }
    if !has_column(conn, "generation_jobs", "status_message")? {
        conn.execute("ALTER TABLE generation_jobs ADD COLUMN status_message TEXT", [])?;
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
        let provider = card.provider.as_deref().unwrap_or("llm");
        tx.execute(
            "INSERT INTO flashcards (id, document_id, chunk_id, question, answer, card_type, source_ref, tags, provider)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                card.id,
                card.document_id,
                card.chunk_id,
                card.question,
                card.answer,
                card.card_type,
                source_ref,
                tags,
                provider,
            ],
        )
        .context("insert flashcard")?;
    }
    tx.commit()?;
    Ok(())
}

pub fn get_flashcards(conn: &Connection, document_id: &str) -> anyhow::Result<Vec<Flashcard>> {
    let mut stmt = conn.prepare(
        "SELECT id, document_id, chunk_id, question, answer, card_type, source_ref, tags, provider
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
            provider: row.get(8).ok(),
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
    pub status_message: Option<String>,
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
        "SELECT id, document_id, status, progress, total, error_message, status_message, density, use_llm
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
            status_message: row.get(6)?,
            density: row.get(7)?,
            use_llm: row.get::<_, i64>(8)? != 0,
        }))
    } else {
        Ok(None)
    }
}

pub fn update_job_progress_with_status(
    conn: &mut Connection,
    job_id: &str,
    progress: i64,
    status_message: &str,
) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE generation_jobs SET progress = ?1, status = ?2, status_message = ?3 WHERE id = ?4",
        params![progress, "generating", status_message, job_id],
    )?;
    Ok(())
}

pub fn set_job_status(
    conn: &mut Connection,
    job_id: &str,
    status: &str,
    error_message: Option<&str>,
    status_message: Option<&str>,
) -> anyhow::Result<()> {
    conn.execute(
        "UPDATE generation_jobs SET status = ?1, error_message = ?2, status_message = ?3 WHERE id = ?4",
        params![status, error_message, status_message, job_id],
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

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_db() -> AppState {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("test.db");
        // Keep the tempdir alive for the life of the app state by leaking it.
        std::mem::forget(dir);
        AppState::new(&db_path).expect("init state")
    }

    fn seed_doc(state: &AppState) -> String {
        let mut conn = state.open().unwrap();
        let doc = StoredDocument {
            id: uuid::Uuid::new_v4().to_string(),
            filename: "Test.pdf".to_string(),
            file_type: "pdf".to_string(),
            page_count: 1,
            total_chars: 100,
            file_hash: uuid::Uuid::new_v4().to_string(),
            storage_key: "key".to_string(),
            status: "parsed".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
            deleted_at: None,
        };
        insert_document(&mut conn, &doc, 42, &[PageContent {
            page_num: 1,
            text: "hello world".to_string(),
            char_offset: 0,
        }])
        .unwrap();
        doc.id
    }

    #[test]
    fn rename_updates_filename() {
        let state = tmp_db();
        let id = seed_doc(&state);

        let mut conn = state.open().unwrap();
        assert!(rename_document(&mut conn, &id, "Renamed.pdf").unwrap());
        let doc = find_document_by_id(&conn, &id).unwrap().unwrap();
        assert_eq!(doc.filename, "Renamed.pdf");
    }

    #[test]
    fn rename_returns_false_for_missing() {
        let state = tmp_db();
        let mut conn = state.open().unwrap();
        assert!(!rename_document(&mut conn, "does-not-exist", "X.pdf").unwrap());
    }

    #[test]
    fn soft_delete_hides_and_trash_lists() {
        let state = tmp_db();
        let id = seed_doc(&state);

        let mut conn = state.open().unwrap();
        assert!(soft_delete_document(&mut conn, &id).unwrap());

        let conn = state.open().unwrap();
        // Hidden from active list
        assert!(list_documents(&conn).unwrap().is_empty());
        // Not found via find_document_by_id
        assert!(find_document_by_id(&conn, &id).unwrap().is_none());
        // Visible in trash
        let trash = list_trash_documents(&conn).unwrap();
        assert_eq!(trash.len(), 1);
        assert_eq!(trash[0].id, id);
        assert!(trash[0].deleted_at.is_some());
    }

    #[test]
    fn soft_delete_twice_returns_false() {
        let state = tmp_db();
        let id = seed_doc(&state);

        let mut conn = state.open().unwrap();
        assert!(soft_delete_document(&mut conn, &id).unwrap());
        assert!(!soft_delete_document(&mut conn, &id).unwrap());
    }

    #[test]
    fn restore_brings_document_back() {
        let state = tmp_db();
        let id = seed_doc(&state);

        let mut conn = state.open().unwrap();
        assert!(soft_delete_document(&mut conn, &id).unwrap());
        assert!(restore_document(&mut conn, &id).unwrap());

        let conn = state.open().unwrap();
        let active = list_documents(&conn).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, id);
        assert!(active[0].deleted_at.is_none());
        assert!(list_trash_documents(&conn).unwrap().is_empty());
    }

    #[test]
    fn restore_missing_returns_false() {
        let state = tmp_db();
        let mut conn = state.open().unwrap();
        assert!(!restore_document(&mut conn, "nope").unwrap());
    }

    #[test]
    fn rename_blocked_while_deleted() {
        let state = tmp_db();
        let id = seed_doc(&state);

        let mut conn = state.open().unwrap();
        assert!(soft_delete_document(&mut conn, &id).unwrap());
        assert!(!rename_document(&mut conn, &id, "TryRename.pdf").unwrap());
    }

    #[test]
    fn find_document_by_hash_including_deleted_finds_soft_deleted_doc() {
        let state = tmp_db();
        let id = seed_doc(&state);

        let mut conn = state.open().unwrap();
        assert!(soft_delete_document(&mut conn, &id).unwrap());

        let conn = state.open().unwrap();
        let doc = find_document_by_hash_including_deleted(&conn, &doc_hash(&state, &id))
            .unwrap()
            .expect("deleted doc should still be findable by hash");
        assert_eq!(doc.id, id);
        assert!(doc.deleted_at.is_some());
    }

    #[test]
    fn restore_document_makes_hash_reusable_for_upload() {
        let state = tmp_db();
        let id = seed_doc(&state);
        let hash = doc_hash(&state, &id);

        let mut conn = state.open().unwrap();
        assert!(soft_delete_document(&mut conn, &id).unwrap());
        assert!(restore_document(&mut conn, &id).unwrap());

        let conn = state.open().unwrap();
        let active = list_documents(&conn).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].file_hash, hash);
    }

    fn doc_hash(state: &AppState, id: &str) -> String {
        let conn = state.open().unwrap();
        let mut stmt = conn
            .prepare("SELECT file_hash FROM documents WHERE id = ?1")
            .unwrap();
        stmt.query_row([id], |row| row.get::<_, String>(0))
            .unwrap()
    }
}
