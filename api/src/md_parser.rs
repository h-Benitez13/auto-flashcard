use crate::models::{DocumentInfo, PageContent};
use std::path::Path;

pub fn parse_markdown(path: &Path) -> Result<DocumentInfo, String> {
    eprintln!("[flashcards] ===============================");
    eprintln!("[flashcards] Parsing Markdown: {:?}", path);

    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.md".to_string());

    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read file: {}", e))?;

    eprintln!("[flashcards] Read {} characters", text.len());

    if text.trim().is_empty() {
        return Err("File is empty.".to_string());
    }

    // Split into pages using ## headings as page breaks
    let sections = split_markdown_sections(&text);
    eprintln!("[flashcards] Found {} sections", sections.len());

    let mut total_chars = 0usize;
    let mut pages = Vec::new();

    for (i, section) in sections.iter().enumerate() {
        pages.push(PageContent {
            page_num: (i + 1) as u32,
            text: section.clone(),
            char_offset: total_chars,
        });
        total_chars += section.len();
    }

    eprintln!("[flashcards] Result: {} pages, {} chars", pages.len(), total_chars);
    eprintln!("[flashcards] ===============================");

    Ok(DocumentInfo {
        id: uuid::Uuid::new_v4().to_string(),
        filename,
        file_type: "md".to_string(),
        page_count: pages.len() as u32,
        total_chars,
        pages,
    })
}

fn split_markdown_sections(text: &str) -> Vec<String> {
    let mut sections = Vec::new();
    let mut current = String::new();
    let mut in_frontmatter = false;

    for line in text.lines() {
        // Handle YAML frontmatter (--- between lines 1-2)
        if line.trim() == "---" && current.is_empty() {
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter && line.trim() == "---" {
            in_frontmatter = false;
            continue;
        }
        if in_frontmatter {
            continue;
        }

        // Split on ## headings (major sections)
        if line.starts_with("## ") {
            if !current.trim().is_empty() {
                sections.push(current.trim().to_string());
            }
            current = String::new();
            current.push_str(line);
            current.push('\n');
        } else if line.starts_with("# ") && !current.trim().is_empty() {
            // H1 headings also split (but not the first one)
            if !current.trim().is_empty() {
                sections.push(current.trim().to_string());
            }
            current = String::new();
            current.push_str(line);
            current.push('\n');
        } else {
            current.push_str(line);
            current.push('\n');
        }
    }

    if !current.trim().is_empty() {
        sections.push(current.trim().to_string());
    }

    // If no headings found, treat whole file as one section
    if sections.is_empty() && !text.trim().is_empty() {
        sections.push(text.trim().to_string());
    }

    sections
}
