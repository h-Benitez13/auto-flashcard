use std::sync::Arc;

use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use sha2::{Digest, Sha256};
use tracing::{error, info};

const MAX_FILE_BYTES: usize = 50 * 1024 * 1024;
const ALLOWED_EXTENSIONS: &[&str] = &["pdf", "md", "markdown", "txt", "pptx", "ppt"];

use serde::Deserialize;

use crate::{
    chunker,
    db::{self, AppState, StoredDocument},
    llm,
    parsers,
};

type ApiError = (StatusCode, Json<serde_json::Value>);

fn err(status: StatusCode, msg: impl Into<String>) -> ApiError {
    (
        status,
        Json(serde_json::json!({ "error": msg.into() })),
    )
}

pub async fn list_documents(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let conn = match state.open() {
        Ok(c) => c,
        Err(e) => return Err(err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };

    match db::list_documents(&conn) {
        Ok(docs) => Ok(Json(docs)),
        Err(e) => {
            error!("Failed to list documents: {}", e);
            Err(err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
        }
    }
}

pub async fn get_document(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let conn = match state.open() {
        Ok(c) => c,
        Err(e) => return Err(err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };

    let doc = match db::find_document_by_id(&conn, &id) {
        Ok(Some(d)) => d,
        Ok(None) => return Err(err(StatusCode::NOT_FOUND, "document not found")),
        Err(e) => return Err(err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };

    let pages = match db::get_pages(&conn, &id) {
        Ok(p) => p,
        Err(e) => return Err(err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };

    Ok(Json(db::to_document_info(&doc, pages)))
}

pub async fn upload(State(state): State<Arc<AppState>>, mut multipart: Multipart) -> impl IntoResponse {
    let mut filename: Option<String> = None;
    let mut data: Option<Vec<u8>> = None;

    loop {
        match multipart.next_field().await {
            Ok(Some(field)) => {
                if field.name().map(|n| n == "file").unwrap_or(false) {
                    filename = field.file_name().map(|s| s.to_string());
                    match field.bytes().await {
                        Ok(bytes) => data = Some(bytes.to_vec()),
                        Err(e) => {
                            return Err(err(
                                StatusCode::BAD_REQUEST,
                                format!("read file: {}", e),
                            ));
                        }
                    }
                    break;
                }
            }
            Ok(None) => break,
            Err(e) => {
                return Err(err(
                    StatusCode::BAD_REQUEST,
                    format!("multipart parse error: {}", e),
                ));
            }
        }
    }

    let filename = filename.unwrap_or_else(|| "upload".to_string());
    let data = match data {
        Some(d) if !d.is_empty() => d,
        _ => return Err(err(StatusCode::BAD_REQUEST, "no file provided")),
    };

    if data.len() > MAX_FILE_BYTES {
        return Err(err(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("file exceeds {} MB limit", MAX_FILE_BYTES / 1024 / 1024),
        ));
    }

    let file_type = detect_file_type(&filename);
    if !ALLOWED_EXTENSIONS.contains(&file_type.as_str()) {
        return Err(err(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            format!("unsupported file type: {}", file_type),
        ));
    }

    info!("Upload received: {} ({} bytes)", filename, data.len());

    let hash = hex::encode(Sha256::digest(&data));

    let conn = match state.open() {
        Ok(c) => c,
        Err(e) => return Err(err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };

    if let Ok(Some(existing)) = db::find_document_by_hash(&conn, &hash) {
        info!("Duplicate upload, returning existing document {}", existing.id);
        let pages = db::get_pages(&conn, &existing.id).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Json(db::to_document_info(&existing, pages)));
    }

    let storage_key = format!("{}_{}", hash, sanitize_filename(&filename));
    let path = state.uploads_dir.join(&storage_key);

    if let Err(e) = tokio::fs::write(&path, &data).await {
        return Err(err(StatusCode::INTERNAL_SERVER_ERROR, format!("save file: {}", e)));
    }

    let path_for_parse = path.clone();
    let file_type_for_parse = file_type.clone();
    let parse_result = tokio::task::spawn_blocking(move || parsers::parse_file(&path_for_parse, &file_type_for_parse))
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, format!("parse task: {}", e)))?;

    let parsed = match parse_result {
        Ok(info) => info,
        Err(e) => {
            let _ = tokio::fs::remove_file(&path).await;
            return Err(err(StatusCode::UNPROCESSABLE_ENTITY, e));
        }
    };

    let doc_id = uuid::Uuid::new_v4().to_string();
    let doc = StoredDocument {
        id: doc_id.clone(),
        filename: filename.clone(),
        file_type: file_type.clone(),
        page_count: parsed.page_count,
        total_chars: parsed.total_chars,
        file_hash: hash.clone(),
        storage_key: storage_key.clone(),
        status: "parsed".to_string(),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    let mut conn = state.open().map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if let Err(e) = db::insert_document(&mut conn, &doc, data.len(), &parsed.pages) {
        let _ = tokio::fs::remove_file(&path).await;
        return Err(err(StatusCode::INTERNAL_SERVER_ERROR, format!("db insert: {}", e)));
    }

    info!(
        "Document {} created with {} pages ({} chars)",
        doc_id, parsed.page_count, parsed.total_chars
    );

    Ok(Json(db::to_document_info(&doc, parsed.pages)))
}

fn detect_file_type(filename: &str) -> String {
    filename
        .rsplit('.')
        .next()
        .map(|ext| ext.to_lowercase())
        .map(|ext| match ext.as_str() {
            "pdf" => "pdf".to_string(),
            "md" | "markdown" | "txt" => "md".to_string(),
            _ => ext,
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn sanitize_filename(name: &str) -> String {
    name.replace(['/', '\\'], "_")
}

// ---------------------------------------------------------------------------
// Flashcard generation
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct GenerateRequest {
    density: Option<String>,
    page_numbers: Option<Vec<u32>>,
}

pub async fn generate_flashcards(
    State(state): State<Arc<AppState>>,
    Path(document_id): Path<String>,
    Json(req): Json<GenerateRequest>,
) -> impl IntoResponse {
    let density = req.density.unwrap_or_else(|| "balanced".to_string());

    let conn = match state.open() {
        Ok(c) => c,
        Err(e) => return Err(err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };

    let _doc = match db::find_document_by_id(&conn, &document_id) {
        Ok(Some(d)) => d,
        Ok(None) => return Err(err(StatusCode::NOT_FOUND, "document not found")),
        Err(e) => return Err(err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };

    let mut pages = match db::get_pages(&conn, &document_id) {
        Ok(p) => p,
        Err(e) => return Err(err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };

    if let Some(selected) = req.page_numbers {
        let selected: std::collections::HashSet<u32> = selected.into_iter().collect();
        pages.retain(|p| selected.contains(&p.page_num));
    }

    if pages.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "no pages selected"));
    }

    let chunks = chunker::chunk_document(&document_id, &pages, 256);
    if chunks.is_empty() {
        return Err(err(StatusCode::BAD_REQUEST, "no chunks could be created"));
    }

    let total = chunks.len();
    let use_llm = std::env::var("GROQ_API_KEY")
        .map(|k| !k.is_empty())
        .unwrap_or(false);

    let mut conn = match state.open() {
        Ok(c) => c,
        Err(e) => return Err(err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };
    if let Err(e) = db::insert_chunks(&mut conn, &chunks) {
        return Err(err(StatusCode::INTERNAL_SERVER_ERROR, format!("insert chunks: {}", e)));
    }

    let job_id = match db::create_job(&mut conn, &document_id, total, &density, use_llm) {
        Ok(id) => id,
        Err(e) => return Err(err(StatusCode::INTERNAL_SERVER_ERROR, format!("create job: {}", e))),
    };

    let state_for_task = Arc::clone(&state);
    let state_for_failure = Arc::clone(&state);
    let job_id_for_response = job_id.clone();
    tokio::spawn(async move {
        let result = generate_task(state_for_task, document_id, job_id.clone(), chunks, density).await;
        if let Err(ref e) = result {
            let msg = e.to_string();
            let job_id2 = job_id.clone();
            let _ = tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
                let mut conn = state_for_failure.open()?;
                db::set_job_status(&mut conn, &job_id2, "failed", Some(&msg))?;
                Ok(())
            }).await;
        }
    });

    Ok(Json(serde_json::json!({
        "job_id": job_id_for_response,
        "total_chunks": total,
        "use_llm": use_llm,
    })))
}

async fn generate_task(
    state: Arc<AppState>,
    document_id: String,
    job_id: String,
    chunks: Vec<crate::models::Chunk>,
    density: String,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let key = std::env::var("GROQ_API_KEY").ok().filter(|k| !k.is_empty());
    let key_ref = key.as_deref();

    let mut all_cards = Vec::new();

    for (idx, chunk) in chunks.iter().enumerate() {
        let cards = llm::generate_for_chunk(&client, key_ref, chunk, Some(&density))
            .await
            .map_err(|e| anyhow::anyhow!(e))?;
        all_cards.extend(cards);

        let progress = (idx + 1) as i64;
        let job_id_clone = job_id.clone();
        let state_for_progress = Arc::clone(&state);
        tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
            let mut conn = state_for_progress.open()?;
            db::update_job_progress(&mut conn, &job_id_clone, progress)?;
            Ok(())
        }).await??;
    }

    let state_for_insert = Arc::clone(&state);
    let job_id_clone = job_id.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        let mut conn = state_for_insert.open()?;
        db::delete_flashcards_for_document(&mut conn, &document_id)?;
        db::insert_flashcards(&mut conn, &all_cards)?;
        db::set_job_status(&mut conn, &job_id_clone, "completed", None)?;
        Ok(())
    }).await??;

    Ok(())
}

pub async fn get_job(
    State(state): State<Arc<AppState>>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let conn = match state.open() {
        Ok(c) => c,
        Err(e) => return Err(err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };

    match db::get_job(&conn, &job_id) {
        Ok(Some(job)) => Ok(Json(job)),
        Ok(None) => Err(err(StatusCode::NOT_FOUND, "job not found")),
        Err(e) => Err(err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub async fn get_document_flashcards(
    State(state): State<Arc<AppState>>,
    Path(document_id): Path<String>,
) -> impl IntoResponse {
    let conn = match state.open() {
        Ok(c) => c,
        Err(e) => return Err(err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    };

    match db::get_flashcards(&conn, &document_id) {
        Ok(cards) => Ok(Json(cards)),
        Err(e) => Err(err(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}
