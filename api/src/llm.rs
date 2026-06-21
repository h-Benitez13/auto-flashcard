use crate::models::{Chunk, Flashcard, SourceRef};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const GROQ_BASE: &str = "https://api.groq.com/openai/v1";
const LLM_MODEL: &str = "llama-3.3-70b-versatile";

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

const WORDS_PER_CARD: f64 = 60.0;
const MIN_CARDS: u32 = 3;
const MAX_CARDS: u32 = 15;

fn density_multiplier(density: &str) -> f64 {
    match density.trim().to_lowercase().as_str() {
        "concise" => 0.6,
        "comprehensive" => 1.6,
        _ => 1.0,
    }
}

fn count_words(text: &str) -> usize {
    text.split_whitespace().count()
}

fn compute_card_target(word_count: usize, multiplier: f64) -> u32 {
    let raw = (word_count as f64 / WORDS_PER_CARD * multiplier).round() as i64;
    raw.clamp(MIN_CARDS as i64, MAX_CARDS as i64) as u32
}

fn max_tokens_for(card_count: u32) -> u32 {
    (card_count * 160 + 256).clamp(512, 4096)
}

fn build_prompt(chunk: &Chunk, card_count: u32) -> String {
    USER_PROMPT_TEMPLATE
        .replace("{startPage}", &chunk.start_page.to_string())
        .replace("{endPage}", &chunk.end_page.to_string())
        .replace("{cardCount}", &card_count.to_string())
        .replace("{chunkText}", &chunk.content)
}

async fn call_llm(
    client: &Client,
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
        let resp = client
            .post(&url)
            .bearer_auth(key)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("LLM request failed: {}", e))?;

        if resp.status().is_success() {
            let chat = resp
                .json::<ChatResponse>()
                .await
                .map_err(|e| format!("Failed to parse LLM response: {}", e))?;
            return chat
                .choices
                .into_iter()
                .next()
                .map(|c| c.message.content)
                .ok_or_else(|| "LLM returned no choices".to_string());
        }

        if resp.status() == 429 && attempt < max_retries {
            let body = resp.text().await.unwrap_or_default();
            let delay_secs = extract_retry_delay(&body).unwrap_or(3.0) + 0.5;

            if delay_secs > 180.0 {
                return Err(format!(
                    "Daily rate limit reached. The API asked us to wait {:.0}s (~{:.0} min).",
                    delay_secs,
                    delay_secs / 60.0
                ));
            }

            tracing::warn!(
                "429 Rate Limit hit (attempt {}/{}). Retrying in {:.1}s...",
                attempt,
                max_retries,
                delay_secs
            );
            tokio::time::sleep(tokio::time::Duration::from_secs_f32(delay_secs)).await;
            continue;
        }

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("LLM API error {}: {}", status, body));
    }

    Err("LLM request failed after max retries".to_string())
}

fn extract_retry_delay(body: &str) -> Option<f32> {
    let needle = "try again in ";
    let start = body.find(needle)?;
    let rest = &body[start + needle.len()..];
    let end = rest.find(['s', 'm', 'h'])?;
    let num_str = &rest[..end];

    if rest[end..].starts_with('m') {
        let minutes: f32 = num_str.parse().ok()?;
        let rest = &rest[end + 1..];
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
        num_str.parse::<f32>().ok()
    }
}

pub async fn generate_for_chunk(
    client: &Client,
    key: Option<&str>,
    chunk: &Chunk,
    density: Option<&str>,
) -> Result<Vec<Flashcard>, String> {
    let density = density.unwrap_or("balanced");

    if key.is_none() {
        return Ok(crate::chunker::generate_flashcards(chunk, Some(density)));
    }

    let key = key.unwrap();
    let multiplier = density_multiplier(density);
    let target = compute_card_target(count_words(&chunk.content), multiplier);

    tracing::info!(
        "LLM generating up to {} cards for chunk {} ({} chars)",
        target,
        chunk.id,
        chunk.content.len()
    );

    let content = call_llm(
        client,
        key,
        build_prompt(chunk, target),
        max_tokens_for(target),
    )
    .await?;

    let cards = parse_llm_response(&content, chunk);
    tracing::info!("Generated {} cards for chunk {}", cards.len(), chunk.id);
    Ok(cards)
}

fn parse_llm_response(text: &str, chunk: &Chunk) -> Vec<Flashcard> {
    let cleaned = text
        .trim()
        .trim_matches(|c: char| c == '`' || c == '\n' || c == ' ');

    let json_str = if cleaned.starts_with("json") {
        cleaned[4..].trim()
    } else if cleaned.starts_with("JSON") {
        cleaned[4..].trim()
    } else {
        cleaned
    };

    if let Ok(llm_cards) = serde_json::from_str::<Vec<LlmCard>>(json_str) {
        let cards: Vec<Flashcard> = llm_cards
            .into_iter()
            .filter_map(|lc| {
                let question = lc.question?;
                let answer = lc.answer?;

                if !answer_grounded_in_source(&answer, &chunk.content) {
                    tracing::warn!("Skipped ungrounded card");
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

    tracing::warn!("JSON parse failed, trying Q/A format");
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
            current_question = strip_prefix_any(line, &["Q:", "Question:", "question:"]);
        } else if (line.starts_with("A:") || line.starts_with("Answer:") || line.starts_with("answer:"))
            && !current_question.is_empty()
        {
            let answer = strip_prefix_any(line, &["A:", "Answer:", "answer:"]);

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

fn strip_prefix_any(line: &str, prefixes: &[&str]) -> String {
    for prefix in prefixes {
        if let Some(rest) = line.strip_prefix(prefix) {
            return rest.trim().to_string();
        }
    }
    line.trim().to_string()
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
        assert_eq!(density_multiplier("  COMPREHENSIVE "), 1.6);
    }

    #[test]
    fn card_target_scales_with_words_and_density() {
        assert_eq!(compute_card_target(10, 1.0), MIN_CARDS);
        let words = 600;
        let concise = compute_card_target(words, density_multiplier("concise"));
        let balanced = compute_card_target(words, density_multiplier("balanced"));
        let comprehensive = compute_card_target(words, density_multiplier("comprehensive"));
        assert!(concise <= balanced && balanced <= comprehensive);
        assert!(comprehensive > concise);
        assert_eq!(compute_card_target(100_000, 1.6), MAX_CARDS);
    }

    #[test]
    fn max_tokens_stays_in_bounds() {
        assert!(max_tokens_for(MIN_CARDS) >= 512);
        assert!(max_tokens_for(MAX_CARDS) <= 4096);
    }
}
