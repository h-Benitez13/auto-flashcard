use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentInfo {
    pub id: String,
    pub filename: String,
    pub file_type: String,
    pub page_count: u32,
    pub total_chars: usize,
    pub pages: Vec<PageContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageContent {
    pub page_num: u32,
    pub text: String,
    pub char_offset: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: String,
    pub document_id: String,
    pub content: String,
    pub token_count: u32,
    pub start_page: u32,
    pub end_page: u32,
    pub start_char: usize,
    pub end_char: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flashcard {
    pub id: String,
    pub document_id: String,
    pub chunk_id: String,
    pub question: String,
    pub answer: String,
    pub card_type: String,
    pub source_ref: SourceRef,
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRef {
    pub page_start: u32,
    pub page_end: u32,
    pub char_start: usize,
    pub char_end: usize,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DocumentSummary {
    pub id: String,
    pub filename: String,
    pub file_type: String,
    pub page_count: u32,
    pub total_chars: usize,
    pub card_count: u32,
}
