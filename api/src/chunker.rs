use crate::models::{Chunk, Flashcard, PageContent, SourceRef};

pub fn chunk_document(
    document_id: &str,
    pages: &[PageContent],
    max_chunk_size: u32,
) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    let mut current_text = String::new();
    let mut current_start_page = 0u32;
    let mut current_start_char = 0usize;
    let mut current_end_page = 0u32;

    for page in pages {
        if current_text.is_empty() {
            current_start_page = page.page_num;
            current_start_char = page.char_offset;
        }

        let char_count = estimate_chars(current_text.len() + page.text.len());
        if char_count > max_chunk_size && !current_text.is_empty() {
            chunks.push(Chunk {
                id: uuid::Uuid::new_v4().to_string(),
                document_id: document_id.to_string(),
                content: current_text.trim().to_string(),
                token_count: estimate_tokens(&current_text),
                start_page: current_start_page,
                end_page: current_end_page.max(current_start_page),
                start_char: current_start_char,
                end_char: current_start_char + current_text.len(),
            });

            current_text = page.text.clone();
            current_start_page = page.page_num;
            current_start_char = page.char_offset;
        } else {
            if !current_text.is_empty() && !page.text.is_empty() {
                current_text.push('\n');
            }
            current_text.push_str(&page.text);
        }

        current_end_page = page.page_num;
    }

    if !current_text.trim().is_empty() {
        chunks.push(Chunk {
            id: uuid::Uuid::new_v4().to_string(),
            document_id: document_id.to_string(),
            content: current_text.trim().to_string(),
            token_count: estimate_tokens(&current_text),
            start_page: current_start_page,
            end_page: current_end_page,
            start_char: current_start_char,
            end_char: current_start_char + current_text.len(),
        });
    }

    chunks
}

fn estimate_chars(char_len: usize) -> u32 {
    (char_len / 4) as u32
}

fn estimate_tokens(text: &str) -> u32 {
    (text.len() / 4) as u32
}

pub fn generate_flashcards(chunk: &Chunk, density: Option<&str>) -> Vec<Flashcard> {
    let terms_per_window = match density.unwrap_or("balanced").trim().to_lowercase().as_str() {
        "concise" => 1,
        "comprehensive" => 4,
        _ => 2,
    };

    let mut cards = Vec::new();
    let sentences = split_sentences(&chunk.content);

    for window in sentences.chunks(2) {
        if window.len() < 2 {
            continue;
        }

        let sentence = window.join(" ");
        if sentence.len() < 40 {
            continue;
        }

        let terms = extract_key_terms(&sentence);
        for term in terms.iter().take(terms_per_window) {
            let question = format!("What is \"{}\"?", term);
            let answer = sentence.clone();

            cards.push(Flashcard {
                id: uuid::Uuid::new_v4().to_string(),
                document_id: chunk.document_id.clone(),
                chunk_id: chunk.id.clone(),
                question,
                answer,
                card_type: "definition".to_string(),
                source_ref: SourceRef {
                    page_start: chunk.start_page,
                    page_end: chunk.end_page,
                    char_start: chunk.start_char,
                    char_end: chunk.end_char,
                    preview: chunk.content.chars().take(200).collect(),
                },
                tags: vec!["auto-generated".to_string()],
            });
        }
    }

    cards
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();

    for c in text.chars() {
        current.push(c);
        if ".!?\n".contains(c) {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                sentences.push(trimmed);
            }
            current.clear();
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        sentences.push(trimmed);
    }

    sentences
}

fn extract_key_terms(text: &str) -> Vec<String> {
    let mut terms = Vec::new();

    let patterns = [" is ", " are ", " refers to ", " defined as "].iter();
    for pattern in patterns {
        if let Some(pos) = text.find(pattern) {
            let before = &text[..pos];
            let words: Vec<&str> = before.split_whitespace().collect();
            if let Some(&last_word) = words.last() {
                let clean = last_word
                    .trim_start_matches(|c: char| c.is_ascii_punctuation())
                    .trim_end_matches(|c: char| c.is_ascii_punctuation());
                if clean.len() > 3 {
                    terms.push(clean.to_string());
                }
            }
        }
    }

    let uppercase_words: Vec<String> = text
        .split_whitespace()
        .filter(|w| {
            w.len() > 3
                && w.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                && w.contains(|c: char| c.is_lowercase())
        })
        .map(|w| w.trim_matches(|c: char| c.is_ascii_punctuation()).to_string())
        .collect();

    for word in uppercase_words {
        if !terms.contains(&word) {
            terms.push(word);
        }
    }

    terms.truncate(5);
    terms
}
