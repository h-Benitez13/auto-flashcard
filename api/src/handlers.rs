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

use crate::{
    db::{self, AppState, StoredDocument},
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
