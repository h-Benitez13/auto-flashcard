use std::path::Path;

use crate::models::DocumentInfo;

pub fn parse_file(path: &Path, file_type: &str) -> Result<DocumentInfo, String> {
    match file_type {
        "pdf" => crate::pdf_parser::parse_pdf(path),
        "md" | "txt" => crate::md_parser::parse_markdown(path),
        _ => Err(format!("Unsupported file type: {}", file_type)),
    }
}
