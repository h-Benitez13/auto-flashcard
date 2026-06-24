use crate::models::{DocumentInfo, PageContent};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

const MAX_PPTX_BYTES: usize = 100 * 1024 * 1024;

/// Parse a .pptx file (Office Open XML presentation).
///
/// A .pptx is a ZIP archive. Slide text lives in `ppt/slides/slideN.xml`
/// inside `<a:t>` text-run elements. Each slide becomes one page.
pub fn parse_pptx(path: &Path) -> Result<DocumentInfo, String> {
    eprintln!("[flashcards] ===============================");
    eprintln!("[flashcards] Parsing PPTX: {:?}", path);

    let filename = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.pptx".to_string());

    let file = File::open(path).map_err(|e| format!("Cannot read PPTX file: {}", e))?;
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    if len > MAX_PPTX_BYTES as u64 {
        return Err(format!(
            "PPTX is too large ({:.1} MB). Max supported: {:.1} MB.",
            len as f64 / 1_048_576.0,
            MAX_PPTX_BYTES as f64 / 1_048_576.0
        ));
    }

    let reader = BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| format!("Failed to open PPTX as ZIP archive: {}", e))?;

    let slide_names = collect_slide_names(&archive)?;
    if slide_names.is_empty() {
        return Err("No slides found in this PPTX. It may be corrupted or empty.".to_string());
    }

    let mut pages = Vec::new();
    let mut total_chars = 0usize;

    for (i, name) in slide_names.iter().enumerate() {
        let mut file = archive
            .by_name(name)
            .map_err(|e| format!("Failed to read {}: {}", name, e))?;

        let mut xml = String::new();
        file
            .read_to_string(&mut xml)
            .map_err(|e| format!("Failed to read slide XML {}: {}", name, e))?;

        let text = extract_slide_text(&xml);
        let clean = clean_text(&text);

        if !clean.is_empty() {
            pages.push(PageContent {
                page_num: (i + 1) as u32,
                text: clean.clone(),
                char_offset: total_chars,
            });
            total_chars += clean.len();
        }
    }

    if pages.is_empty() {
        return Err(
            "PPTX parsed but no text was extracted.\n\
             This deck may contain only images, diagrams, or scanned content."
                .to_string(),
        );
    }

    eprintln!(
        "[flashcards] Result: {} slides, {} chars",
        pages.len(),
        total_chars
    );
    eprintln!("[flashcards] ===============================");

    Ok(DocumentInfo {
        id: uuid::Uuid::new_v4().to_string(),
        filename,
        file_type: "pptx".to_string(),
        page_count: pages.len() as u32,
        total_chars,
        pages,
    })
}

fn collect_slide_names<R: Read + std::io::Seek>(
    archive: &zip::ZipArchive<R>,
) -> Result<Vec<String>, String> {
    let mut slides: Vec<(u32, String)> = Vec::new();

    for name in archive.file_names() {
        if name.starts_with("ppt/slides/slide") && name.ends_with(".xml") {
            // Extract the number from "ppt/slides/slide42.xml"
            let num_part = &name["ppt/slides/slide".len()..name.len() - 4];
            match num_part.parse::<u32>() {
                Ok(n) => slides.push((n, name.to_string())),
                Err(_) => {
                    // Unexpected naming; still include with a high sort key so it doesn't break order
                    slides.push((u32::MAX, name.to_string()))
                }
            }
        }
    }

    slides.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(slides.into_iter().map(|(_, name)| name).collect())
}

fn extract_slide_text(xml: &str) -> String {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    // Join text runs within a paragraph with spaces, but keep paragraphs
    // separated by newlines so slide titles/bullets retain their structure.
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current_paragraph_runs: Vec<String> = Vec::new();
    let mut current_run = String::new();
    let mut in_text_run = false;
    let mut in_paragraph = false;
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                if e.name().as_ref() == b"a:p" {
                    in_paragraph = true;
                    current_paragraph_runs.clear();
                } else if e.name().as_ref() == b"a:t" {
                    in_text_run = true;
                    current_run.clear();
                }
            }
            Ok(Event::Text(e)) => {
                if in_text_run {
                    if let Ok(t) = e.unescape() {
                        current_run.push_str(&t);
                    }
                }
            }
            Ok(Event::End(e)) => {
                if e.name().as_ref() == b"a:t" {
                    in_text_run = false;
                    if !current_run.is_empty() {
                        current_paragraph_runs.push(current_run.clone());
                    }
                } else if e.name().as_ref() == b"a:p" {
                    in_paragraph = false;
                    let para_text = current_paragraph_runs.join(" ").trim().to_string();
                    if !para_text.is_empty() {
                        paragraphs.push(para_text);
                    }
                    current_paragraph_runs.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => break,
            _ => {}
        }
        buf.clear();
    }

    // Flush any paragraph that was not closed.
    if in_paragraph && !current_paragraph_runs.is_empty() {
        let para_text = current_paragraph_runs.join(" ").trim().to_string();
        if !para_text.is_empty() {
            paragraphs.push(para_text);
        }
    }

    paragraphs.join("\n")
}

fn clean_text(text: &str) -> String {
    text.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<&str>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_slide_text_collects_a_t_elements() {
        let xml = r#"<?xml version="1.0"?>
            <p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main">
                <a:p><a:r><a:t>Hello</a:t></a:r><a:r><a:t>world</a:t></a:r></a:p>
                <a:p><a:r><a:t>Second paragraph.</a:t></a:r></a:p>
            </p:sld>"#;

        let text = extract_slide_text(xml);
        assert_eq!(text, "Hello world\nSecond paragraph.");
    }

    #[test]
    fn collect_slide_names_sorts_numerically() {
        use std::io::{Cursor, Write};

        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(Cursor::new(&mut buf));
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);

            for name in [
                "ppt/slides/slide10.xml",
                "ppt/slides/slide2.xml",
                "ppt/slides/slide1.xml",
                "ppt/_rels/presentation.xml.rels",
            ] {
                zip.start_file(name, options).unwrap();
                zip.write_all(b"<xml/>").unwrap();
            }
            zip.finish().unwrap();
        }

        let cursor = Cursor::new(buf);
        let archive = zip::ZipArchive::new(cursor).unwrap();
        let names = collect_slide_names(&archive).unwrap();

        assert_eq!(
            names,
            vec![
                "ppt/slides/slide1.xml".to_string(),
                "ppt/slides/slide2.xml".to_string(),
                "ppt/slides/slide10.xml".to_string(),
            ]
        );
    }

    #[test]
    fn clean_text_collapses_blank_lines() {
        let input = "  hello  \n\n\n  world  ";
        assert_eq!(clean_text(input), "hello\nworld");
    }
}
