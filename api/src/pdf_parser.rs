use crate::models::{DocumentInfo, PageContent};
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use std::process::Command;

const PDF_TIMEOUT_SECONDS: u64 = 30;

// ---------------------------------------------------------------------------
// Public API — orchestrator
// ---------------------------------------------------------------------------

pub fn parse_pdf(path: &Path) -> Result<DocumentInfo, String> {
    eprintln!("[flashcards] ===============================");
    eprintln!("[flashcards] Parsing PDF: {:?}", path);

    let bytes = read_file(path)?;

    let (md_text, method) = extract_pdf_to_markdown(path, &bytes)?;
    eprintln!("[flashcards] Extraction method: {}", method);

    if md_text.trim().is_empty() {
        return Err(
            "PDF parsed but no text was extracted.\n\
             This PDF may be scanned (image-based), encrypted, or contain only images.\n\
             Try using OCR or converting to a text-based PDF first."
                .to_string(),
        );
    }

    let info = build_document_info(&md_text, &path.file_name().unwrap_or_default().to_string_lossy());

    eprintln!("[flashcards] Result: {} pages, {} chars", info.page_count, info.total_chars);
    eprintln!("[flashcards] ===============================");

    Ok(info)
}

// ---------------------------------------------------------------------------
// Public API — individually testable units
// ---------------------------------------------------------------------------

pub fn read_file(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = fs::read(path)
        .map_err(|e| format!("Cannot read PDF file at path: {}\nError: {}", path.display(), e))?;

    eprintln!("[flashcards] Read {} bytes ({:.1} MB)", bytes.len(), bytes.len() as f64 / 1_048_576.0);

    if bytes.len() < 10 {
        return Err(format!("PDF file appears empty ({} bytes)", bytes.len()));
    }

    if bytes.len() > 100_000_000 {
        return Err(format!(
            "PDF is too large ({:.1} MB). Max supported: 100 MB.\n\
             Try splitting the PDF or converting to a smaller file.",
            bytes.len() as f64 / 1_048_576.0
        ));
    }

    Ok(bytes)
}

pub fn extract_pdf_to_markdown(path: &Path, bytes: &[u8]) -> Result<(String, String), String> {
    // Try pdftotext first (best quality for text-based PDFs)
    eprintln!("[flashcards] Attempting pdftotext conversion...");
    match extract_with_pdftotext(path) {
        Ok(text) => {
            eprintln!("[flashcards] pdftotext extracted {} characters (Markdown)", text.len());
            Ok((text, "pdftotext".into()))
        }
        Err(e) => {
            eprintln!("[flashcards] pdftotext: {}. Trying lopdf...", e);
            match extract_with_timeout(bytes, extract_with_lopdf) {
                Ok(text) => {
                    eprintln!("[flashcards] lopdf extracted {} characters", text.len());
                    Ok((text, "lopdf".into()))
                }
                Err(e2) => {
                    eprintln!("[flashcards] lopdf: {}. Trying pdf-extract...", e2);
                    match extract_with_timeout(bytes, extract_with_pdf_extract) {
                        Ok(text) => {
                            eprintln!("[flashcards] pdf-extract extracted {} characters", text.len());
                            Ok((text, "pdf-extract".into()))
                        }
                        Err(e3) => {
                            eprintln!("[flashcards] pdf-extract: {}", e3);
                            eprintln!("[flashcards] All 3 extraction methods failed for this PDF.");
                            Err(format!(
                                "PDF extraction failed. All methods tried:\n\
                                 1. pdftotext: {}\n\
                                 2. lopdf: {}\n\
                                 3. pdf-extract: {}",
                                e, e2, e3
                            ))
                        }
                    }
                }
            }
        }
    }
}

pub fn build_document_info(text: &str, filename: &str) -> DocumentInfo {
    let page_texts = split_into_pages(text);
    eprintln!("[flashcards] Split into {} pages", page_texts.len());

    let mut total_chars = 0usize;
    let mut pages = Vec::new();

    for (i, page_text) in page_texts.iter().enumerate() {
        let clean = clean_text(page_text);
        pages.push(PageContent {
            page_num: (i + 1) as u32,
            text: clean.clone(),
            char_offset: total_chars,
        });
        total_chars += clean.len();
    }

    DocumentInfo {
        id: uuid::Uuid::new_v4().to_string(),
        filename: filename.to_string(),
        file_type: "pdf".to_string(),
        page_count: pages.len() as u32,
        total_chars,
        pages,
    }
}

// ---------------------------------------------------------------------------
// Private helpers — extraction strategy chain
// ---------------------------------------------------------------------------

fn extract_with_pdftotext(path: &Path) -> Result<String, String> {
    // Check if pdftotext (from poppler) is available
    let check = Command::new("which")
        .arg("pdftotext")
        .output();

    if check.is_err() || !check.unwrap().status.success() {
        return Err("pdftotext not installed (install via: brew install poppler)".to_string());
    }

    // Run pdftotext with -layout to preserve structure, output to stdout via "-"
    let output = Command::new("pdftotext")
        .arg("-layout")
        .arg(path.as_os_str())
        .arg("-")
        .output()
        .map_err(|e| format!("pdftotext execution failed: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pdftotext error: {}", stderr.trim()));
    }

    let raw_text = String::from_utf8_lossy(&output.stdout).to_string();
    let markdown = plaintext_to_markdown(&raw_text);
    Ok(markdown)
}

fn extract_with_timeout<F>(bytes: &[u8], extractor: F) -> Result<String, String>
where
    F: FnOnce(&[u8]) -> Result<String, String> + Send + 'static,
{
    let bytes_owned = bytes.to_vec();
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        let result = extractor(&bytes_owned);
        let _ = tx.send(result);
    });

    match rx.recv_timeout(Duration::from_secs(PDF_TIMEOUT_SECONDS)) {
        Ok(result) => result,
        Err(_) => Err(format!(
            "PDF extraction timed out after {} seconds.\n\
             The PDF may be too complex or contain many images.",
            PDF_TIMEOUT_SECONDS
        )),
    }
}

fn extract_with_lopdf(bytes: &[u8]) -> Result<String, String> {
    use lopdf::Document;
    
    let doc = Document::load_mem(bytes)
        .map_err(|e| format!("lopdf failed to load: {}", e))?;

    let mut all_text = String::new();
    let pages = doc.get_pages();
    let mut page_nums: Vec<u32> = pages.keys().copied().collect();
    page_nums.sort();

    for page_num in page_nums {
        if let Some(page_id) = pages.get(&page_num) {
            if let Ok(page_text) = extract_page_text_lopdf(&doc, *page_id) {
                if !page_text.trim().is_empty() {
                    all_text.push_str(&page_text);
                    all_text.push('\n');
                    all_text.push('\n');
                }
            }
        }
    }

    Ok(all_text)
}

fn extract_page_text_lopdf(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> Result<String, String> {
    use lopdf::Object;

    let page_obj = doc.get_object(page_id)
        .map_err(|e| format!("Failed to get page object: {}", e))?;
    let dict = page_obj.as_dict()
        .map_err(|e| format!("Failed to get page dict: {}", e))?;

    let content = dict.get(b"Contents")
        .map_err(|e| format!("No Contents: {}", e))?;

    let mut stream = match content {
        Object::Reference(r) => {
            let obj = doc.get_object(*r)
                .map_err(|e| format!("Failed to resolve reference: {}", e))?;
            obj.as_stream()
                .map_err(|e| format!("Not a stream: {}", e))?
                .clone()
        }
        obj => obj.as_stream()
            .map_err(|e| format!("Not a stream: {}", e))?
            .clone()
    };

    stream.decompress()
        .map_err(|e| format!("Failed to decompress: {}", e))?;

    let raw = String::from_utf8_lossy(&stream.content);
    let text = extract_text_operators(&raw);

    Ok(text)
}

fn extract_text_operators(content: &str) -> String {
    let mut result = String::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        // TJ operator (array of strings and numbers)
        if line.ends_with("TJ") {
            extract_tj_text(line, &mut result);
            continue;
        }

        // Tj operator (single string in parentheses)
        if line.ends_with("Tj") {
            extract_tj_single(line, &mut result);
            continue;
        }

        // ' operator (move to next line and show text)
        if line.ends_with("'") {
            extract_apos_text(line, &mut result);
            continue;
        }

        // " operator (move to next line and show text with word spacing)
        if line.ends_with("\"") {
            extract_quote_text(line, &mut result);
            continue;
        }
    }

    if result.is_empty() {
        // Fallback: extract any text in parentheses
        extract_parentheses_text(content, &mut result);
    }

    result
}

fn extract_tj_text(line: &str, result: &mut String) {
    // Format: [(text1) num1 (text2) num2 ...] TJ
    if let Some(start) = line.find('[') {
        if let Some(end) = line.rfind("]") {
            let array_content = &line[start + 1..end];
            let mut in_text = false;
            let mut current_text = String::new();

            for ch in array_content.chars() {
                match ch {
                    '(' if !in_text => {
                        in_text = true;
                        current_text.clear();
                    }
                    '(' if in_text => {
                        current_text.push('(');
                    }
                    ')' if in_text => {
                        in_text = false;
                        result.push_str(&unescape_pdf_text(&current_text));
                    }
                    _ if in_text => {
                        current_text.push(ch);
                    }
                    _ => {} // Skip numbers/spaces outside parens
                }
            }
        }
    }
}

fn extract_tj_single(line: &str, result: &mut String) {
    // Format: (text) Tj
    if let Some(start) = line.find('(') {
        if let Some(end) = line.rfind(")") {
            let text = &line[start + 1..end];
            result.push_str(&unescape_pdf_text(text));
        }
    }
}

fn extract_apos_text(line: &str, result: &mut String) {
    // Format: x y (text) '
    if let Some(start) = line.rfind('(') {
        if let Some(end) = line.rfind("')'") {
            let text = &line[start + 1..end];
            result.push(' ');
            result.push_str(&unescape_pdf_text(text));
        }
    }
}

fn extract_quote_text(line: &str, result: &mut String) {
    // Format: x y aw ac (text) "
    if let Some(start) = line.rfind('(') {
        if let Some(end) = line.rfind(")\"") {
            let text = &line[start + 1..end];
            result.push(' ');
            result.push_str(&unescape_pdf_text(text));
        }
    }
}

fn extract_parentheses_text(content: &str, result: &mut String) {
    let mut in_text = false;
    let mut current = String::new();
    let mut paren_depth = 0;

    for ch in content.chars() {
        match ch {
            '(' if !in_text => {
                in_text = true;
                paren_depth = 1;
                current.clear();
            }
            '(' if in_text => {
                paren_depth += 1;
                current.push('(');
            }
            ')' if in_text => {
                paren_depth -= 1;
                if paren_depth == 0 {
                    in_text = false;
                    let clean = unescape_pdf_text(&current);
                    if !clean.trim().is_empty() {
                        result.push_str(&clean);
                    }
                } else {
                    current.push(')');
                }
            }
            '\\' if in_text => {
                // Skip escape character - handled by unescape_pdf_text
                current.push('\\');
            }
            _ if in_text => {
                current.push(ch);
            }
            _ => {}
        }
    }
}

fn unescape_pdf_text(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    'b' => result.push('\u{0008}'),
                    'f' => result.push('\u{000C}'),
                    '(' => result.push('('),
                    ')' => result.push(')'),
                    '\\' => result.push('\\'),
                    '\n' => {} // Line continuation
                    d if d.is_ascii_digit() => {
                        // Octal escape \ddd
                        let mut octal = String::new();
                        octal.push(d);
                        for _ in 0..2 {
                            if let Some(c) = chars.next() {
                                if c.is_ascii_digit() {
                                    octal.push(c);
                                } else {
                                    break;
                                }
                            }
                        }
                        if let Ok(val) = u32::from_str_radix(&octal, 8) {
                            if let Some(c) = std::char::from_u32(val) {
                                result.push(c);
                            }
                        }
                    }
                    _ => result.push(next),
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

fn extract_with_pdf_extract(bytes: &[u8]) -> Result<String, String> {
    pdf_extract::extract_text_from_mem(bytes)
        .map_err(|e| format!("pdf-extract failed: {}", e))
}

// ---------------------------------------------------------------------------
// Private helpers — text processing
// ---------------------------------------------------------------------------

/// Convert raw pdftotext -layout output into clean Markdown.
/// Splits on form-feed characters first to preserve page boundaries,
/// then detects headings, list items (○/■/●), and collapses whitespace.
fn plaintext_to_markdown(raw: &str) -> String {
    // Split on form feeds first — pdftotext uses \x0C between pages.
    // Process each page independently, then rejoin with \x0C.
    let page_segments: Vec<&str> = raw.split('\u{000C}').collect();
    let mut pages: Vec<String> = Vec::new();

    for segment in &page_segments {
        let converted = convert_page_to_markdown(segment);
        if !converted.trim().is_empty() {
            pages.push(converted);
        }
    }

    pages.join("\u{000C}")
}

/// Convert a single page of pdftotext output to Markdown.
fn convert_page_to_markdown(page_text: &str) -> String {
    let mut md = String::new();
    let mut prev_blank = false;

    for line in page_text.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() {
            if !prev_blank {
                md.push('\n');
            }
            prev_blank = true;
            continue;
        }
        prev_blank = false;

        // Detect numbered headings (e.g. "1.  Define profession.")
        let is_numbered_heading = {
            let mut chars = trimmed.chars();
            chars.next().map_or(false, |c| c.is_ascii_digit())
                && trimmed.contains('.')
                && trimmed.len() < 120
                && !trimmed.contains('\u{25CB}')  // ○
                && !trimmed.contains('\u{25A0}')  // ■
                && !trimmed.contains('\u{25CF}')  // ●
        };

        // Detect short standalone lines that look like titles/headings
        let is_heading = trimmed.len() < 80
            && !trimmed.contains('\u{25CB}')
            && !trimmed.contains('\u{25A0}')
            && !trimmed.contains('\u{25CF}')
            && !trimmed.starts_with('-')
            && (
                (trimmed.len() > 3 && trimmed == trimmed.to_uppercase() && trimmed.chars().any(|c| c.is_alphabetic()))
                || is_numbered_heading
            );

        if is_heading {
            md.push_str("\n## ");
            md.push_str(trimmed);
            md.push('\n');
            continue;
        }

        // Convert bullet-like prefixes to Markdown list syntax
        let converted = if let Some(rest) = trimmed.strip_prefix('\u{25CB}') {
            format!("- {}", rest.trim())
        } else if let Some(rest) = trimmed.strip_prefix('\u{25A0}') {
            format!("  - {}", rest.trim())
        } else if let Some(rest) = trimmed.strip_prefix('\u{25CF}') {
            format!("    - {}", rest.trim())
        } else {
            trimmed.to_string()
        };

        md.push_str(&converted);
        md.push('\n');
    }

    md
}

fn clean_text(text: &str) -> String {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<&str>>()
        .join("\n")
}

fn split_into_pages(text: &str) -> Vec<String> {
    let mut pages = Vec::new();
    let mut current = String::new();

    // Normalize form feeds that appear mid-line: treat \x0C as a line break
    // before splitting by lines. This handles pandoc output where \x0C may
    // appear inline (e.g., "Page1\x0CPage2") or on its own line.
    for segment in text.split('\u{000C}') {
        for line in segment.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            current.push_str(trimmed);
            current.push('\n');
        }
        if !current.trim().is_empty() {
            pages.push(current.trim().to_string());
        }
        current = String::new();
    }

    pages
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- build_document_info ------------------------------------------------

    #[test]
    fn test_build_document_info_single_page() {
        let text = "Hello world\n\nThis is a test.";
        let info = build_document_info(text, "test.pdf");
        assert_eq!(info.page_count, 1);
        assert_eq!(info.filename, "test.pdf");
        assert_eq!(info.file_type, "pdf");
        assert!(info.total_chars > 0);
        assert_eq!(info.pages.len(), 1);
        assert!(!info.id.is_empty());
    }

    #[test]
    fn test_build_document_info_with_form_feeds() {
        let text = "Page 1 content\x0CPage 2 content";
        let info = build_document_info(text, "multi.pdf");
        assert_eq!(info.page_count, 2);
        assert_eq!(info.pages.len(), 2);
        assert_eq!(info.pages[0].page_num, 1);
        assert_eq!(info.pages[1].page_num, 2);
        assert_eq!(info.pages[0].char_offset, 0);
        assert!(info.pages[1].char_offset > 0);
    }

    #[test]
    fn test_build_document_info_empty_text() {
        let text = "   ";
        let info = build_document_info(text, "empty.pdf");
        assert_eq!(info.page_count, 0);
        assert_eq!(info.total_chars, 0);
    }

    #[test]
    fn test_build_document_info_multiple_form_feeds() {
        let text = "A\x0CB\x0CC\x0CD";
        let info = build_document_info(text, "four.pdf");
        assert_eq!(info.page_count, 4);
    }

    #[test]
    fn test_build_document_info_preserves_content() {
        let text = "The mitochondria is the powerhouse of the cell.";
        let info = build_document_info(text, "bio.pdf");
        assert!(info.pages[0].text.contains("mitochondria"));
    }

    // -- clean_text ---------------------------------------------------------

    #[test]
    fn test_clean_text_removes_blank_lines() {
        let input = "  hello  \n  \n  world  ";
        assert_eq!(clean_text(input), "hello\nworld");
    }

    #[test]
    fn test_clean_text_trims_whitespace() {
        let input = "   spaced   \n   out   ";
        assert_eq!(clean_text(input), "spaced\nout");
    }

    #[test]
    fn test_clean_text_empty() {
        assert_eq!(clean_text(""), "");
        assert_eq!(clean_text("   \n   "), "");
    }

    #[test]
    fn test_clean_text_single_line() {
        assert_eq!(clean_text("  just one line  "), "just one line");
    }

    // -- split_into_pages ---------------------------------------------------

    #[test]
    fn test_split_into_pages_no_form_feeds() {
        let pages = split_into_pages("Just some continuous text\nwith multiple\nlines.");
        assert_eq!(pages.len(), 1);
        assert!(pages[0].contains("continuous text"));
    }

    #[test]
    fn test_split_into_pages_with_form_feeds() {
        let pages = split_into_pages("Page1\x0CPage2\x0CPage3");
        assert_eq!(pages.len(), 3);
        assert!(pages[0].contains("Page1"));
        assert!(pages[1].contains("Page2"));
        assert!(pages[2].contains("Page3"));
    }

    #[test]
    fn test_split_into_pages_empty() {
        let pages = split_into_pages("");
        assert!(pages.is_empty());
    }

    #[test]
    fn test_split_into_pages_whitespace_only() {
        let pages = split_into_pages("   \n   ");
        assert!(pages.is_empty());
    }

    #[test]
    fn test_split_into_pages_empty_page_between_feeds() {
        // Two form feeds with nothing between = only the non-empty page
        let pages = split_into_pages("Content\x0C\x0CMore content");
        assert_eq!(pages.len(), 2);
    }

    #[test]
    fn test_split_into_pages_preserves_double_newlines() {
        let text = "Paragraph one.\n\nParagraph two.\x0CPage two.";
        let pages = split_into_pages(text);
        assert_eq!(pages.len(), 2);
        assert!(pages[0].contains("Paragraph one."));
        assert!(pages[0].contains("Paragraph two."));
    }

    // -- unescape_pdf_text --------------------------------------------------

    #[test]
    fn test_unescape_pdf_text_basic_escapes() {
        assert_eq!(unescape_pdf_text("hello\\nworld"), "hello\nworld");
        assert_eq!(unescape_pdf_text("a\\rb"), "a\rb");
        assert_eq!(unescape_pdf_text("a\\tb"), "a\tb");
    }

    #[test]
    fn test_unescape_pdf_text_parens() {
        assert_eq!(unescape_pdf_text("\\(hello\\)"), "(hello)");
    }

    #[test]
    fn test_unescape_pdf_text_no_escapes() {
        assert_eq!(unescape_pdf_text("plain text"), "plain text");
    }

    // -- read_file ----------------------------------------------------------

    #[test]
    fn test_read_file_nonexistent() {
        let result = read_file(Path::new("/nonexistent/path.pdf"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cannot read PDF"));
    }
}
