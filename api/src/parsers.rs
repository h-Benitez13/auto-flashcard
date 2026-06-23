use std::path::Path;

use crate::models::DocumentInfo;

pub fn parse_file(path: &Path, file_type: &str) -> Result<DocumentInfo, String> {
    match file_type {
        "pdf" => crate::pdf_parser::parse_pdf(path),
        "md" | "txt" => crate::md_parser::parse_markdown(path),
        "pptx" => crate::pptx_parser::parse_pptx(path),
        "ppt" => Err("Legacy .ppt (binary PowerPoint) is not supported. \
            Please save the file as .pptx (File > Save As > PowerPoint Presentation) \
            and upload again."
            .to_string()),
        _ => Err(format!("Unsupported file type: {}", file_type)),
    }
}
