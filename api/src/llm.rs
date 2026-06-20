use crate::models::{Chunk, Flashcard, SourceRef};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::Emitter;

// Groq's OpenAI-compatible API. The key is baked in at build time from the
// GROQ_API_KEY env var (see build.rs `rerun-if-env-changed`); it is never committed.
const GROQ_BASE: &str = "https://api.groq.com/openai/v1";
const LLM_MODEL: &str = "llama-3.3-70b-versatile";
const MAX_PARALLEL: usize = 3;

/// Returns the build-time API key, or None if it wasn't provided / is empty.
fn api_key() -> Option<&'static str> {
    match option_env!("GROQ_API_KEY") {
        Some(k) if !k.is_empty() => Some(k),
        _ => None,
    }
}

const SYSTEM_PROMPT: &str = r#"You are a flashcard generator. Your ONLY job is to create question-answer pairs DIRECTLY from the provided source text.

RULES:
1. Every answer MUST be completely derivable from the source text alone
2. Do NOT add outside knowledge, explanations, or inferences
3. Use the EXACT terminology from the source
4. Keep answers concise but complete
5. Each card must test a DISTINCT idea — never repeat or rephrase the same question
6. Only output a JSON array, nothing else

OUTPUT FORMAT:
[
  {
    "question": "What is [term]?",
    "answer": "[exact answer from source]",
    "card_type": "definition",
    "tags": ["topic"]
  }
]

CARD TYPES: definition, comparison, list, mechanism, clinical, value
Only create cards where both question and answer are EXPLICITLY in the source text."#;

const USER_PROMPT_TEMPLATE: &str = r#"SOURCE TEXT (pages {startPage}-{endPage}):
---
{chunkText}
---

Create up to {cardCount} flashcards from THIS SOURCE ONLY. Cover as many DISTINCT concepts, definitions, facts, processes, comparisons, and values as the text genuinely supports — without repeating, rephrasing, or padding. If the text only supports fewer solid cards, return fewer. Return a JSON array of cards."#;

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    temperature: f32,
    top_p: f32,
    max_tokens: u32,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Deserialize)]
struct ChatResponseMessage {
    content: String,
}

#[derive(Deserialize, Debug)]
struct LlmCard {
    question: Option<String>,
    answer: Option<String>,
    card_type: Option<String>,
    tags: Option<Vec<String>>,
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .connect_timeout(Duration::from_secs(5))
        .build()
        .expect("Failed to build HTTP client")
}

#[tauri::command]
pub fn check_llm() -> Result<String, String> {
    let key = match api_key() {
        Some(k) => k,
        None => {
            eprintln!("[flashcards] No GROQ_API_KEY baked into this build");
            return Ok("unavailable".to_string());
        }
    };

    let client = client();
    match client
        .get(&format!("{}/models", GROQ_BASE))
        .bearer_auth(key)
        .timeout(Duration::from_secs(4))
        .send()
    {
        Ok(resp) if resp.status().is_success() => {
            eprintln!("[flashcards] LLM API reachable");
            // Key present and API reachable -> LLM generation is available.
            Ok("connected:true".to_string())
        }
        Ok(resp) => {
            eprintln!("[flashcards] LLM API returned status {}", resp.status());
            Ok("unavailable".to_string())
        }
        Err(e) => {
            eprintln!("[flashcards] LLM API not reachable: {}", e);
            Ok("unavailable".to_string())
        }
    }
}

/// Build the user prompt for a single chunk, requesting `card_count` cards.
fn build_prompt(chunk: &Chunk, card_count: u32) -> String {
    USER_PROMPT_TEMPLATE
        .replace("{startPage}", &chunk.start_page.to_string())
        .replace("{endPage}", &chunk.end_page.to_string())
        .replace("{cardCount}", &card_count.to_string())
        .replace("{chunkText}", &chunk.content)
}

// --- Quantity scaling -------------------------------------------------------
// The number of cards requested per chunk scales with how much text the chunk
// contains, multiplied by a user-selected density. This is what makes
// "generate from all pages" produce a card count proportional to the content
// instead of a flat 3-5 per chunk.

/// Approximate words of source that justify one flashcard (at balanced density).
const WORDS_PER_CARD: f64 = 60.0;
const MIN_CARDS: u32 = 3;
const MAX_CARDS: u32 = 15;

/// Map a density preset to a card-count multiplier.
fn density_multiplier(density: &str) -> f64 {
    match density.trim().to_lowercase().as_str() {
        "concise" => 0.6,
        "comprehensive" => 1.6,
        _ => 1.0, // "balanced" / unknown -> default
    }
}

fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

/// Target number of cards for a chunk given its word count and density multiplier.
fn compute_card_target(word_count: usize, multiplier: f64) -> u32 {
    let raw = (word_count as f64 / WORDS_PER_CARD * multiplier).round() as i64;
    raw.clamp(MIN_CARDS as i64, MAX_CARDS as i64) as u32
}

/// Token budget for the response, scaled to the requested card count.
fn max_tokens_for(card_count: u32) -> u32 {
    (card_count * 160 + 256).clamp(512, 4096)
}

/// Normalize a question for duplicate detection (lowercase, alphanumeric words only).
/// Call the Groq chat-completions API with retry-on-429 logic.
/// Emits `llm:rate-limited` events to the frontend when a rate limit is hit.
fn call_llm(
    app: &tauri::AppHandle,
    client: &reqwest::blocking::Client,
    key: &str,
    prompt: String,
    max_tokens: u32,
) -> Result<String, String> {
    let request = ChatRequest {
        model: LLM_MODEL.to_string(),
        messages: vec![
            ChatMessage {
                role: "system".to_string(),
                content: SYSTEM_PROMPT.to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: prompt,
            },
        ],
        temperature: 0.3,
        top_p: 0.9,
        max_tokens,
    };

    let url = format!("{}/chat/completions", GROQ_BASE);
    let max_retries = 3;

    for attempt in 1..=max_retries {
        let resp = match client
            .post(&url)
            .bearer_auth(key)
            .json(&request)
            .send()
        {
            Ok(r) => r,
            Err(e) => {
                return Err(format!("LLM request failed: {}", e));
            }
        };

        if resp.status().is_success() {
            let chat = resp
                .json::<ChatResponse>()
                .map_err(|e| format!("Failed to parse LLM response: {}", e))?;

            return chat.choices
                .into_iter()
                .next()
                .map(|c| c.message.content)
                .ok_or_else(|| "LLM returned no choices".to_string());
        }

        if resp.status() == 429 && attempt < max_retries {
            let body = resp.text().unwrap_or_default();
            let delay_secs = extract_retry_delay(&body).unwrap_or(3.0) + 0.5;

            // Fail fast if the API asks us to wait an unreasonable amount of time
            // (e.g. Groq free-tier TPD limit → 20+ minute waits). Return a fatal
            // error so the frontend can display it instead of hanging silently.
            if delay_secs > 180.0 {
                return Err(format!(
                    "Daily rate limit reached. The API asked us to wait {:.0}s (~{:.0} min). Please try again later.",
                    delay_secs, delay_secs / 60.0
                ));
            }

            eprintln!(
                "[flashcards] 429 Rate Limit hit (attempt {}/{}). Retrying in {:.1}s...",
                attempt, max_retries, delay_secs
            );

            let _ = app.emit("llm:rate-limited", serde_json::json!({
                "delay": delay_secs,
                "attempt": attempt,
                "max_retries": max_retries,
            }));

            std::thread::sleep(std::time::Duration::from_secs_f32(delay_secs));
            continue;
        }

        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        return Err(format!("LLM API error {}: {}", status, body));
    }

    Err("LLM request failed after max retries".to_string())
}

/// Extract the retry delay (in seconds) from a Groq 429 error body.
/// Handles formats like "2.53s", "2m53.664s", "1h2m30s".
fn extract_retry_delay(body: &str) -> Option<f32> {
    let needle = "try again in ";
    let start = body.find(needle)?;
    let rest = &body[start + needle.len()..];
    let end = rest.find(['s', 'm', 'h'])?;
    let num_str = &rest[..end];

    // Check if the next character after the number is 'm' (minutes)
    if rest[end..].starts_with('m') {
        let minutes: f32 = num_str.parse().ok()?;
        let rest = &rest[end + 1..];
        // Try to parse seconds after the 'm'
        if let Some(sec_end) = rest.find('s') {
            let seconds: f32 = rest[..sec_end].parse().unwrap_or(0.0);
            Some(minutes * 60.0 + seconds)
        } else {
            Some(minutes * 60.0)
        }
    } else if rest[end..].starts_with('h') {
        let hours: f32 = num_str.parse().ok()?;
        Some(hours * 3600.0)
    } else {
        // Plain seconds: "2.53s"
        num_str.parse::<f32>().ok()
    }
}

#[tauri::command]
pub fn generate_with_llm(app: tauri::AppHandle, chunk: Chunk, density: Option<String>) -> Result<Vec<Flashcard>, String> {
    let multiplier = density_multiplier(density.as_deref().unwrap_or("balanced"));
    let target = compute_card_target(count_words(&chunk.content), multiplier);
    eprintln!("[flashcards] LLM generating up to {} cards for chunk {} ({} chars)",
              target, chunk.id, chunk.content.len());

    let key = api_key().ok_or_else(|| "No API key configured in this build".to_string())?;
    let client = client();
    let content = call_llm(&app, &client, key, build_prompt(&chunk, target), max_tokens_for(target))?;

    let cards = parse_llm_response(&content, &chunk);
    eprintln!("[flashcards] Generated {} cards", cards.len());

    Ok(cards)
}

/// Process multiple chunks in parallel (up to MAX_PARALLEL at a time).
#[tauri::command]
pub fn generate_batch_with_llm(
    app: tauri::AppHandle,
    chunks: Vec<Chunk>,
    density: Option<String>,
) -> Result<Vec<Flashcard>, String> {
    let multiplier = density_multiplier(density.as_deref().unwrap_or("balanced"));
    eprintln!("[flashcards] Batch LLM generation: {} chunks, parallelism {}, density x{:.1}",
              chunks.len(), MAX_PARALLEL, multiplier);

    let key = api_key().ok_or_else(|| "No API key configured in this build".to_string())?;
    let client = Arc::new(client());
    let mut all_cards: Vec<Flashcard> = Vec::new();

    // Process in batches of MAX_PARALLEL
    for batch in chunks.chunks(MAX_PARALLEL) {
        let handles: Vec<_> = batch
            .iter()
            .cloned()
            .map(|chunk| {
                let client = Arc::clone(&client);
                let app_clone = app.clone();
                thread::spawn(move || {
                    let target = compute_card_target(count_words(&chunk.content), multiplier);
                    eprintln!("[flashcards] LLM generating up to {} cards for chunk {} ({} chars)",
                              target, chunk.id, chunk.content.len());

                    let content = match call_llm(&app_clone, &client, key, build_prompt(&chunk, target), max_tokens_for(target)) {
                        Ok(c) => c,
                        Err(e) => return Err(e),
                    };
                    let cards = parse_llm_response(&content, &chunk);
                    eprintln!("[flashcards] Chunk {} → {} cards", chunk.id, cards.len());
                    Ok::<Vec<Flashcard>, String>(cards)
                })
            })
            .collect();

        for handle in handles {
            match handle.join() {
                Ok(Ok(cards)) => all_cards.extend(cards),
                Ok(Err(e)) => eprintln!("[flashcards] Chunk failed: {}", e),
                Err(_) => eprintln!("[flashcards] Chunk thread panicked"),
            }
        }
    }

    eprintln!("[flashcards] Batch complete: {} total cards", all_cards.len());
    Ok(all_cards)
}

fn parse_llm_response(text: &str, chunk: &Chunk) -> Vec<Flashcard> {
    let cleaned = text.trim()
        .trim_matches(|c: char| c == '`' || c == '\n' || c == ' ');

    let json_str = if cleaned.starts_with("json") {
        cleaned[4..].trim()
    } else if cleaned.starts_with("JSON") {
        cleaned[4..].trim()
    } else {
        cleaned
    };

    // Try JSON array first
    if let Ok(llm_cards) = serde_json::from_str::<Vec<LlmCard>>(json_str) {
        let cards: Vec<Flashcard> = llm_cards
            .into_iter()
            .filter_map(|lc| {
                let question = lc.question?;
                let answer = lc.answer?;

                if !answer_grounded_in_source(&answer, &chunk.content) {
                    eprintln!("[flashcards] Skipped ungrounded card");
                    return None;
                }

                Some(Flashcard {
                    id: uuid::Uuid::new_v4().to_string(),
                    document_id: chunk.document_id.clone(),
                    chunk_id: chunk.id.clone(),
                    question,
                    answer,
                    card_type: lc.card_type.unwrap_or_else(|| "definition".to_string()),
                    source_ref: SourceRef {
                        page_start: chunk.start_page,
                        page_end: chunk.end_page,
                        char_start: chunk.start_char,
                        char_end: chunk.end_char,
                        preview: chunk.content.chars().take(200).collect(),
                    },
                    tags: lc.tags.unwrap_or_default(),
                })
            })
            .collect();

        if !cards.is_empty() {
            return cards;
        }
    }

    // Fallback: try parsing Q:/A: format
    eprintln!("[flashcards] JSON parse failed, trying Q/A format");
    extract_cards_from_q_a(text, chunk)
}

fn answer_grounded_in_source(answer: &str, source: &str) -> bool {
    let source_lower = source.to_lowercase();
    let answer_words: Vec<&str> = answer
        .split_whitespace()
        .filter(|w| {
            let w = w.trim_matches(|c: char| c.is_ascii_punctuation());
            w.len() > 3
        })
        .collect();

    if answer_words.is_empty() {
        return true;
    }

    let matched = answer_words
        .iter()
        .filter(|w| {
            let w = w.trim_matches(|c: char| c.is_ascii_punctuation());
            source_lower.contains(&w.to_lowercase())
        })
        .count();

    let ratio = matched as f64 / answer_words.len() as f64;
    ratio >= 0.5
}

fn extract_cards_from_q_a(text: &str, chunk: &Chunk) -> Vec<Flashcard> {
    let mut cards = Vec::new();
    let mut current_question = String::new();

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with("Q:") || line.starts_with("Question:") {
            current_question = line
                .trim_start_matches(|c: char| c == 'Q' || c == 'u' || c == 'e' || c == 's' || c == 't' || c == 'i' || c == 'o' || c == 'n' || c == ':')
                .trim()
                .to_string();
            // Fix the trim: remove just "Q:" or "Question:" prefix
            if current_question == line.trim() {
                // The trim_start_matches ate too much, reconstruct
                current_question = line
                    .strip_prefix("Q:")
                    .or_else(|| line.strip_prefix("Question:"))
                    .or_else(|| line.strip_prefix("question:"))
                    .unwrap_or(line)
                    .trim()
                    .to_string();
            }
        } else if (line.starts_with("A:") || line.starts_with("Answer:") || line.starts_with("answer:"))
            && !current_question.is_empty()
        {
            let answer = line
                .strip_prefix("A:")
                .or_else(|| line.strip_prefix("Answer:"))
                .or_else(|| line.strip_prefix("answer:"))
                .unwrap_or(line)
                .trim()
                .to_string();

            if !answer.is_empty() && answer_grounded_in_source(&answer, &chunk.content) {
                cards.push(Flashcard {
                    id: uuid::Uuid::new_v4().to_string(),
                    document_id: chunk.document_id.clone(),
                    chunk_id: chunk.id.clone(),
                    question: current_question.clone(),
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
            current_question.clear();
        }
    }

    cards
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn density_multiplier_presets() {
        assert!(density_multiplier("concise") < 1.0);
        assert_eq!(density_multiplier("balanced"), 1.0);
        assert!(density_multiplier("comprehensive") > 1.0);
        assert_eq!(density_multiplier("unknown"), 1.0);
        // Case/whitespace insensitive.
        assert_eq!(density_multiplier("  COMPREHENSIVE "), 1.6);
    }

    #[test]
    fn card_target_scales_with_words_and_density() {
        // Short text floors at MIN_CARDS.
        assert_eq!(compute_card_target(10, 1.0), MIN_CARDS);
        // For the same text, comprehensive >= balanced >= concise.
        let words = 600;
        let concise = compute_card_target(words, density_multiplier("concise"));
        let balanced = compute_card_target(words, density_multiplier("balanced"));
        let comprehensive = compute_card_target(words, density_multiplier("comprehensive"));
        assert!(concise <= balanced && balanced <= comprehensive);
        assert!(comprehensive > concise);
        // Very large text caps at MAX_CARDS.
        assert_eq!(compute_card_target(100_000, 1.6), MAX_CARDS);
    }

    #[test]
    fn max_tokens_stays_in_bounds() {
        assert!(max_tokens_for(MIN_CARDS) >= 512);
        assert!(max_tokens_for(MAX_CARDS) <= 4096);
    }
}
