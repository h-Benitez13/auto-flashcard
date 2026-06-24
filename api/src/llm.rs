use crate::models::{Chunk, Flashcard, SourceRef};
use reqwest::Client;
use serde::{Deserialize, Serialize};

const SYSTEM_PROMPT: &str = r#"You are an expert study-aid creator. Generate flashcards from the provided source text that would actually help a student prepare for an exam.

RULES:
1. Generate ONLY from the provided source text. Do NOT add outside knowledge, explanations, or inferences.
2. Ask varied, exam-style questions: definitions, lists, comparisons, mechanisms/processes, causes/effects, "why"/"how" questions, fill-in-the-blank, and true/false rephrased as questions.
3. NEVER default to generic "What is {word}?" questions. Use "What is [term]?" ONLY when the source explicitly defines that term. Prefer specific, contextual questions.
4. Each card must test ONE distinct idea. Never repeat or rephrase the same question.
5. Use the EXACT terminology from the source.
6. Keep answers concise but complete, and fully derivable from the source.
7. Only output a JSON array, nothing else.

OUTPUT FORMAT:
[
  {
    "question": "According to the text, what are the three main components of X?",
    "answer": "The three main components are A, B, and C.",
    "card_type": "list",
    "tags": ["topic"]
  },
  {
    "question": "How does X lead to Y?",
    "answer": "X causes Y by doing Z.",
    "card_type": "mechanism",
    "tags": ["topic"]
  },
  {
    "question": "Compare A and B.",
    "answer": "A does X, while B does Y.",
    "card_type": "comparison",
    "tags": ["topic"]
  },
  {
    "question": "___________ is defined as the process by which plants convert light into energy.",
    "answer": "Photosynthesis",
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

Create up to {cardCount} high-quality flashcards from THIS SOURCE ONLY. Focus on concepts, facts, and relationships that could appear on a test. Ask diverse question types and avoid repeating the same phrasing. If the text supports fewer solid cards, return fewer. Return ONLY a JSON array of cards."#;

// ---------------------------------------------------------------------------
// Provider chain — all providers use the OpenAI-compatible /chat/completions API
// ---------------------------------------------------------------------------

pub struct Provider {
    pub name: &'static str,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

/// Build the provider chain from env vars, in priority order.
/// Providers without a key are skipped.
/// Order: Groq (free) → Cerebras (free) → OpenAI (paid safety net).
pub fn build_providers() -> Vec<Provider> {
    let mut providers = Vec::new();

    if let Some(key) = std::env::var("GROQ_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
    {
        providers.push(Provider {
            name: "groq",
            base_url: "https://api.groq.com/openai/v1".to_string(),
            model: "llama-3.3-70b-versatile".to_string(),
            api_key: key,
        });
    }

    if let Some(key) = std::env::var("CEREBRAS_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
    {
        providers.push(Provider {
            name: "cerebras",
            base_url: "https://api.cerebras.ai/v1".to_string(),
            model: "gpt-oss-120b".to_string(),
            api_key: key,
        });
    }

    if let Some(key) = std::env::var("OPENAI_API_KEY")
        .ok()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
    {
        providers.push(Provider {
            name: "openai",
            base_url: "https://api.openai.com/v1".to_string(),
            model: "gpt-4o-mini".to_string(),
            api_key: key,
        });
    }

    providers
}

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
    content: Option<String>,
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
const MAX_CARDS: u32 = 30;

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
    // Reasoning models (Cerebras gpt-oss-120b, zai-glm-4.7) spend many tokens
    // on internal reasoning before producing card output, so we use a generous
    // budget. 16384 is safe for Groq llama-3.3-70b, OpenAI gpt-4o-mini, and
    // Cerebras reasoning models.
    ((card_count * 160 + 256) * 3).clamp(2048, 16384)
}

fn build_prompt(chunk: &Chunk, card_count: u32) -> String {
    USER_PROMPT_TEMPLATE
        .replace("{startPage}", &chunk.start_page.to_string())
        .replace("{endPage}", &chunk.end_page.to_string())
        .replace("{cardCount}", &card_count.to_string())
        .replace("{chunkText}", &chunk.content)
}

/// Call a single provider. Retries on per-minute 429s.
/// Returns Err with "Daily rate limit" in the message if the wait exceeds 180s,
/// so the caller can skip to the next provider in the chain.
async fn call_provider(
    client: &Client,
    provider: &Provider,
    prompt: String,
    max_tokens: u32,
) -> Result<String, String> {
    let request = ChatRequest {
        model: provider.model.clone(),
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

    let url = format!("{}/chat/completions", provider.base_url);
    let max_retries = 3;

    for attempt in 1..=max_retries {
        let resp = client
            .post(&url)
            .bearer_auth(&provider.api_key)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("[{}] request failed: {}", provider.name, e))?;

        if resp.status().is_success() {
            let body = resp
                .text()
                .await
                .map_err(|e| format!("[{}] read body: {}", provider.name, e))?;
            let chat: ChatResponse = serde_json::from_str(&body).map_err(|e| {
                format!(
                    "[{}] parse response: {} (body: {})",
                    provider.name,
                    e,
                    &body[..body.len().min(300)]
                )
            })?;
            return chat
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.message.content)
                .filter(|c| !c.is_empty())
                .ok_or_else(|| {
                    format!(
                        "[{}] returned empty content (likely truncated by reasoning tokens)",
                        provider.name
                    )
                });
        }

        if resp.status() == 429 && attempt < max_retries {
            let body = resp.text().await.unwrap_or_default();

            // Cerebras returns "token_quota_exceeded" for tokens-per-minute limits
            // without a "try again in" delay — default to 60s for per-minute quotas.
            let delay_secs = if body.contains("token_quota_exceeded")
                || body.contains("too_many_tokens")
                || body.contains("Tokens per minute")
            {
                60.0
            } else {
                extract_retry_delay(&body).unwrap_or(3.0) + 0.5
            };

            if delay_secs > 180.0 {
                return Err(format!(
                    "Daily rate limit reached on {}. Wait {:.0}s (~{:.0} min).",
                    provider.name,
                    delay_secs,
                    delay_secs / 60.0
                ));
            }

            tracing::warn!(
                "[{}] 429 Rate Limit (attempt {}/{}). Retrying in {:.1}s...",
                provider.name,
                attempt,
                max_retries,
                delay_secs
            );
            tokio::time::sleep(tokio::time::Duration::from_secs_f32(delay_secs)).await;
            continue;
        }

        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("[{}] API error {}: {}", provider.name, status, body));
    }

    Err(format!("[{}] request failed after max retries", provider.name))
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

/// Try each provider in order. First success returns cards.
/// Daily-limit errors skip to the next provider immediately.
/// Returns Err only if all providers fail.
pub async fn generate_for_chunk(
    client: &Client,
    providers: &[Provider],
    chunk: &Chunk,
    density: Option<&str>,
) -> Result<Vec<Flashcard>, String> {
    let density = density.unwrap_or("balanced");

    if providers.is_empty() {
        return Ok(filter_and_deduplicate_cards(crate::chunker::generate_flashcards(
            chunk,
            Some(density),
        )));
    }

    let multiplier = density_multiplier(density);
    let target = compute_card_target(count_words(&chunk.content), multiplier);
    let prompt = build_prompt(chunk, target);
    let max_tokens = max_tokens_for(target);

    let mut last_err = String::new();

    for provider in providers {
        tracing::info!(
            "[{}] generating up to {} cards for chunk {} ({} chars)",
            provider.name,
            target,
            chunk.id,
            chunk.content.len()
        );

        match call_provider(client, provider, prompt.clone(), max_tokens).await {
            Ok(content) => {
                let cards = parse_llm_response(&content, chunk);
                tracing::info!(
                    "[{}] generated {} cards for chunk {}",
                    provider.name,
                    cards.len(),
                    chunk.id
                );
                return Ok(cards);
            }
            Err(e) => {
                tracing::warn!(
                    "[{}] failed for chunk {}: {} — trying next provider",
                    provider.name,
                    chunk.id,
                    e
                );
                last_err = e;
            }
        }
    }

    Err(format!("All providers failed. Last error: {}", last_err))
}

fn parse_llm_response(text: &str, chunk: &Chunk) -> Vec<Flashcard> {
    let cleaned = text
        .trim()
        .trim_matches(|c: char| c == '`' || c == '\n' || c == ' ');

    let json_str = if let Some(rest) = cleaned.strip_prefix("json") {
        rest.trim()
    } else if let Some(rest) = cleaned.strip_prefix("JSON") {
        rest.trim()
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

        let cards = filter_and_deduplicate_cards(cards);
        if !cards.is_empty() {
            return cards;
        }
        // JSON parsed but every card was filtered out (low quality / duplicate).
        tracing::warn!("All LLM cards filtered out (low quality or duplicates)");
        return Vec::new();
    }

    tracing::warn!("JSON parse failed, trying Q/A format");
    filter_and_deduplicate_cards(extract_cards_from_q_a(text, chunk))
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

fn is_low_quality_card(question: &str, answer: &str) -> bool {
    let q = question.trim();
    let a = answer.trim();

    if q.is_empty() || a.is_empty() {
        return true;
    }

    if q.to_lowercase() == a.to_lowercase() {
        return true;
    }

    // Fill-in-the-blank cards intentionally have short term answers (e.g. "ATP",
    // "Mitochondria"). Only enforce a tiny minimum for non-blank answers.
    let is_fill_in_blank = q.contains("___");
    if !is_fill_in_blank && a.len() < 3 {
        return true;
    }

    false
}

fn normalize_question(question: &str) -> String {
    question
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn filter_and_deduplicate_cards(cards: Vec<Flashcard>) -> Vec<Flashcard> {
    let mut seen = std::collections::HashSet::new();
    cards
        .into_iter()
        .filter(|c| {
            if is_low_quality_card(&c.question, &c.answer) {
                tracing::warn!("Skipped low-quality card: {:?}", c.question);
                return false;
            }
            let key = normalize_question(&c.question);
            if key.is_empty() || !seen.insert(key) {
                tracing::warn!("Skipped duplicate card: {:?}", c.question);
                return false;
            }
            true
        })
        .collect()
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
        assert!(max_tokens_for(MIN_CARDS) >= 2048);
        assert!(max_tokens_for(MAX_CARDS) <= 16384);
    }

    #[test]
    fn build_providers_includes_all_when_keys_set() {
        std::env::set_var("GROQ_API_KEY", "test-groq");
        std::env::set_var("CEREBRAS_API_KEY", "test-cerebras");
        std::env::set_var("OPENAI_API_KEY", "test-openai");

        let providers = build_providers();
        assert_eq!(providers.len(), 3);
        assert_eq!(providers[0].name, "groq");
        assert_eq!(providers[1].name, "cerebras");
        assert_eq!(providers[2].name, "openai");

        std::env::remove_var("GROQ_API_KEY");
        std::env::remove_var("CEREBRAS_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");
    }

    #[test]
    fn build_providers_skips_empty_keys() {
        std::env::set_var("GROQ_API_KEY", "test-groq");
        std::env::set_var("CEREBRAS_API_KEY", "");
        std::env::remove_var("OPENAI_API_KEY");

        let providers = build_providers();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].name, "groq");

        std::env::remove_var("GROQ_API_KEY");
    }

    #[test]
    fn build_providers_empty_when_no_keys() {
        std::env::remove_var("GROQ_API_KEY");
        std::env::remove_var("CEREBRAS_API_KEY");
        std::env::remove_var("OPENAI_API_KEY");

        let providers = build_providers();
        assert!(providers.is_empty());
    }

    #[test]
    fn build_providers_trims_whitespace() {
        std::env::set_var("GROQ_API_KEY", "  spaced-key  ");
        let providers = build_providers();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].api_key, "spaced-key");
        std::env::remove_var("GROQ_API_KEY");
    }

    #[test]
    fn low_quality_filter_rejects_empty_or_duplicate() {
        assert!(is_low_quality_card("What is X?", ""));
        assert!(is_low_quality_card("Same text", "Same text"));
    }

    #[test]
    fn low_quality_filter_accepts_real_definition_not_starting_with_term() {
        // A valid definition often restates the concept without literally
        // starting with the term.
        assert!(!is_low_quality_card(
            "What is photosynthesis?",
            "The process by which plants convert light into energy."
        ));
    }

    #[test]
    fn low_quality_filter_accepts_short_term_answers() {
        // Fill-in-the-blank and short-definition answers should survive.
        assert!(!is_low_quality_card("What is RNA?", "Ribonucleic acid."));
        assert!(!is_low_quality_card(
            "___________ is the powerhouse of the cell.",
            "Mitochondria"
        ));
        assert!(!is_low_quality_card(
            "What is the mitochondria?",
            "Powerhouse of the cell."
        ));
    }

    #[test]
    fn normalize_question_strips_punctuation_and_case() {
        assert_eq!(
            normalize_question("  What is Photosynthesis?! "),
            "what is photosynthesis"
        );
        assert_eq!(normalize_question("Compare A and B:"), "compare a and b");
    }

    #[test]
    fn filter_and_deduplicate_keeps_first_drops_duplicates_and_low_quality() {
        let _chunk = crate::models::Chunk {
            id: "chunk-1".to_string(),
            document_id: "doc-1".to_string(),
            content: "source text".to_string(),
            token_count: 2,
            start_page: 1,
            end_page: 1,
            start_char: 0,
            end_char: 11,
        };
        let cards = vec![
            crate::models::Flashcard {
                id: "a".to_string(),
                document_id: "doc-1".to_string(),
                chunk_id: "chunk-1".to_string(),
                question: "What is X?".to_string(),
                answer: "X is a thing.".to_string(),
                card_type: "definition".to_string(),
                source_ref: crate::models::SourceRef {
                    page_start: 1,
                    page_end: 1,
                    char_start: 0,
                    char_end: 11,
                    preview: "source text".to_string(),
                },
                tags: vec![],
            },
            crate::models::Flashcard {
                id: "b".to_string(),
                document_id: "doc-1".to_string(),
                chunk_id: "chunk-1".to_string(),
                question: "What is x?".to_string(),
                answer: "Y is a thing.".to_string(),
                card_type: "definition".to_string(),
                source_ref: crate::models::SourceRef {
                    page_start: 1,
                    page_end: 1,
                    char_start: 0,
                    char_end: 11,
                    preview: "source text".to_string(),
                },
                tags: vec![],
            },
            crate::models::Flashcard {
                id: "c".to_string(),
                document_id: "doc-1".to_string(),
                chunk_id: "chunk-1".to_string(),
                question: "".to_string(),
                answer: "bad".to_string(),
                card_type: "definition".to_string(),
                source_ref: crate::models::SourceRef {
                    page_start: 1,
                    page_end: 1,
                    char_start: 0,
                    char_end: 11,
                    preview: "source text".to_string(),
                },
                tags: vec![],
            },
        ];
        let filtered = filter_and_deduplicate_cards(cards);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, "a");
    }
}
