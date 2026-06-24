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

/// Chunk a PowerPoint deck into small slide groups so each chunk stays focused
/// on a coherent set of slides. This preserves slide boundaries and improves
/// question quality compared to packing many slides into one large chunk.
pub fn chunk_pptx(document_id: &str, pages: &[PageContent], max_slides_per_chunk: usize) -> Vec<Chunk> {
    let max_slides = max_slides_per_chunk.max(1);
    pages
        .chunks(max_slides)
        .map(|window| {
            let content = window
                .iter()
                .map(|p| p.text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            let token_count = estimate_tokens(&content);
            let content_len = content.len();
            let start_page = window.first().map(|p| p.page_num).unwrap_or(1);
            let end_page = window.last().map(|p| p.page_num).unwrap_or(start_page);
            let start_char = window.first().map(|p| p.char_offset).unwrap_or(0);
            Chunk {
                id: uuid::Uuid::new_v4().to_string(),
                document_id: document_id.to_string(),
                content,
                token_count,
                start_page,
                end_page,
                start_char,
                end_char: start_char + content_len,
            }
        })
        .collect()
}

pub fn generate_flashcards(chunk: &Chunk, density: Option<&str>) -> Vec<Flashcard> {
    let target = match density.unwrap_or("balanced").trim().to_lowercase().as_str() {
        "concise" => 1,
        "comprehensive" => 4,
        _ => 2,
    };

    let sentences = split_sentences(&chunk.content);
    let mut cards = Vec::new();
    let mut used_terms = std::collections::HashSet::new();

    // Prefer definitional sentences with varied question phrasing.
    for sentence in sentences.iter().filter(|s| s.len() >= 25) {
        if cards.len() >= target {
            break;
        }

        if let Some((term, definition)) = extract_definition(sentence) {
            if used_terms.insert(term.to_lowercase()) {
                let template = cards.len() % 3;
                let question = match template {
                    0 => format!("According to the source, what is {}?", term),
                    1 => format!("{} is ___________.", term),
                    _ => format!("What does the source say {} refers to?", term),
                };
                cards.push(make_flashcard(
                    question,
                    definition.to_string(),
                    "definition",
                    chunk,
                ));
            }
            continue;
        }

        // Look for explicit list/count patterns.
        if let Some((question, answer)) = maybe_list_card(sentence) {
            if used_terms.insert(normalize(&question)) {
                cards.push(make_flashcard(question, answer, "list", chunk));
            }
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

fn extract_definition(sentence: &str) -> Option<(&str, &str)> {
    // Longer/more specific patterns first so we don't split on bare "is" prematurely.
    let patterns = [
        " is defined as ",
        " are defined as ",
        " refers to ",
        " refer to ",
        " consists of ",
        " consist of ",
        " means ",
        " is ",
        " are ",
    ];

    for pattern in patterns {
        if let Some(pos) = sentence.find(pattern) {
            let before = sentence[..pos].trim_end();
            let after = sentence[pos + pattern.len()..].trim_start();
            if after.is_empty() {
                continue;
            }
            let term = before
                .split_whitespace()
                .last()?
                .trim_matches(|c: char| c.is_ascii_punctuation());
            if term.len() > 2 {
                return Some((term, after));
            }
        }
    }

    None
}

fn maybe_list_card(sentence: &str) -> Option<(String, String)> {
    let lower = sentence.to_lowercase();
    let count_words = ["two", "three", "four", "five", "six", "seven", "eight", "nine", "ten"];
    let list_nouns = ["main", "types", "components", "causes", "factors", "steps", "stages"];

    for word in count_words {
        for noun in list_nouns {
            let marker = format!("{} {}", word, noun);
            if lower.contains(&marker) {
                let question = if noun == "main" {
                    format!("According to the source, what are the {} main things?", word)
                } else {
                    format!("According to the source, what are the {} {}?", word, noun)
                };
                return Some((question, sentence.to_string()));
            }
        }
    }

    None
}

fn make_flashcard(question: String, answer: String, card_type: &str, chunk: &Chunk) -> Flashcard {
    Flashcard {
        id: uuid::Uuid::new_v4().to_string(),
        document_id: chunk.document_id.clone(),
        chunk_id: chunk.id.clone(),
        question,
        answer,
        card_type: card_type.to_string(),
        source_ref: SourceRef {
            page_start: chunk.start_page,
            page_end: chunk.end_page,
            char_start: chunk.start_char,
            char_end: chunk.end_char,
            preview: chunk.content.chars().take(200).collect(),
        },
        tags: vec!["fallback".to_string()],
    }
}

fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_chunk(content: &str) -> Chunk {
        Chunk {
            id: "chunk-1".to_string(),
            document_id: "doc-1".to_string(),
            content: content.to_string(),
            token_count: estimate_tokens(content),
            start_page: 1,
            end_page: 1,
            start_char: 0,
            end_char: content.len(),
        }
    }

    fn test_pages(texts: &[&str]) -> Vec<PageContent> {
        let mut offset = 0usize;
        texts
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let page = PageContent {
                    page_num: (i + 1) as u32,
                    text: text.to_string(),
                    char_offset: offset,
                };
                offset += text.len() + 1;
                page
            })
            .collect()
    }

    #[test]
    fn chunk_pptx_groups_slides() {
        let pages = test_pages(&[
            "Slide 1 title\nBullet A",
            "Slide 2 title\nBullet B",
            "Slide 3 title\nBullet C",
            "Slide 4 title\nBullet D",
        ]);
        let chunks = chunk_pptx("doc-1", &pages, 2);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].start_page, 1);
        assert_eq!(chunks[0].end_page, 2);
        assert_eq!(chunks[1].start_page, 3);
        assert_eq!(chunks[1].end_page, 4);
        assert!(chunks[0].content.contains("Slide 1"));
        assert!(chunks[0].content.contains("Slide 2"));
    }

    #[test]
    fn fallback_prefers_definitions_and_varies_phrasing() {
        let chunk = test_chunk("Photosynthesis is the process by which plants convert light into energy. Cellular respiration releases stored energy.");
        let cards = generate_flashcards(&chunk, Some("balanced"));
        assert!(!cards.is_empty());
        assert!(cards.iter().all(|c| !c.question.contains("\"What is")));
        // Should get at least one definition-style card about Photosynthesis.
        assert!(cards
            .iter()
            .any(|c| c.question.to_lowercase().contains("photosynthesis")));
    }

    #[test]
    fn fallback_does_not_grab_arbitrary_capitalized_words() {
        let chunk = test_chunk("Revenue grew this quarter. Profits increased in every region.");
        let cards = generate_flashcards(&chunk, Some("balanced"));
        // No definitional patterns in this text, so the rule-based fallback
        // should not invent definition cards from capitalized words.
        assert!(cards.is_empty());
    }

    #[test]
    fn extract_definition_finds_term_and_definition() {
        let sentence = "Mitochondria are the powerhouse of the cell.";
        let (term, def) = extract_definition(sentence).unwrap();
        assert_eq!(term, "Mitochondria");
        assert_eq!(def, "the powerhouse of the cell.");
    }

    #[test]
    fn maybe_list_card_generates_grammatical_question() {
        let sentence = "There are three main causes of the disease.";
        let (question, _) = maybe_list_card(sentence).unwrap();
        assert!(question.contains("three main things"), "question was: {}", question);

        let sentence2 = "The four types of tissue are epithelial, connective, muscle, and nervous.";
        let (question2, _) = maybe_list_card(sentence2).unwrap();
        assert!(question2.contains("four types"), "question was: {}", question2);
    }
}
