use regex::Regex;
use std::sync::LazyLock;
use serde_json::{json, Value};
use crate::storage::clipboard::ClipboardEntry;
use crate::storage::credentials::CredentialsProvider;

static RE_ITEM: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^paste\s+(?:the\s+)?(?:item|clip|number)\s+([a-zA-Z0-9]+)$").unwrap()
});

static RE_AGO: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^paste\s+(?:the\s+)?(?:thing|clip|item)?\s*(?:that\s+)?i\s+copied\s+([a-zA-Z0-9]+)\s+items?\s+ago$").unwrap()
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClipboardRecallIntent {
    /// e.g. "paste item 3", "paste the thing I copied 2 items ago", "paste last clip"
    RelativeIndex(usize),
    /// e.g. "paste the URL", "paste that email", "paste the code"
    Category(String),
    /// e.g. "paste the SQL query with join", "paste the docker command"
    SemanticDescription(String),
}

fn word_to_number(word: &str) -> Option<usize> {
    match word.to_lowercase().as_str() {
        "1" | "one" | "first" | "1st" => Some(1),
        "2" | "two" | "second" | "2nd" => Some(2),
        "3" | "three" | "third" | "3rd" => Some(3),
        "4" | "four" | "fourth" | "4th" => Some(4),
        "5" | "five" | "fifth" | "5th" => Some(5),
        "6" | "six" | "sixth" | "6th" => Some(6),
        "7" | "seven" | "seventh" | "7th" => Some(7),
        "8" | "eight" | "eighth" | "8th" => Some(8),
        "9" | "nine" | "ninth" | "9th" => Some(9),
        "10" | "ten" | "tenth" | "10th" => Some(10),
        s => s.parse::<usize>().ok(),
    }
}

/// Detect if the user's spoken text is a clipboard recall command
pub fn detect_clipboard_recall_intent(spoken_text: &str) -> Option<ClipboardRecallIntent> {
    let raw = spoken_text.trim();
    let stripped = raw
        .trim_end_matches(['.', ',', '!', '?', ';', ':'])
        .trim();
    let lower = stripped.to_lowercase();

    if !lower.starts_with("paste") {
        return None;
    }

    // 1. "paste last clip", "paste the last thing I copied", "paste previous clip"
    if lower == "paste last clip"
        || lower == "paste the last clip"
        || lower == "paste previous clip"
        || lower == "paste the previous clip"
        || lower == "paste the last thing i copied"
        || lower == "paste last thing i copied"
        || lower == "paste what i just copied"
        || lower == "paste the last copied item"
    {
        return Some(ClipboardRecallIntent::RelativeIndex(1));
    }

    // 2. "paste item <N>", "paste clip <N>", "paste number <N>"
    if let Some(caps) = RE_ITEM.captures(&lower) {
        if let Some(matched) = caps.get(1) {
            if let Some(num) = word_to_number(matched.as_str()) {
                if num >= 1 && num <= 50 {
                    return Some(ClipboardRecallIntent::RelativeIndex(num));
                }
            }
        }
    }

    // 3. "paste the thing I copied <N> items ago" / "paste the clip I copied <N> items ago"
    if let Some(caps) = RE_AGO.captures(&lower) {
        if let Some(matched) = caps.get(1) {
            if let Some(num) = word_to_number(matched.as_str()) {
                if num >= 1 && num <= 50 {
                    return Some(ClipboardRecallIntent::RelativeIndex(num));
                }
            }
        }
    }

    // 4. Exact Category match: "paste the URL", "paste that email", "paste the code"
    if lower == "paste the url" || lower == "paste that url" || lower == "paste the link" || lower == "paste that link" || lower == "paste the website" {
        return Some(ClipboardRecallIntent::Category("url".to_string()));
    }
    if lower == "paste the email" || lower == "paste that email" || lower == "paste the email address" || lower == "paste that email address" {
        return Some(ClipboardRecallIntent::Category("email".to_string()));
    }
    if lower == "paste the code" || lower == "paste that code" || lower == "paste the code snippet" || lower == "paste the script" {
        return Some(ClipboardRecallIntent::Category("code".to_string()));
    }
    if lower == "paste the file path" || lower == "paste that file path" || lower == "paste the path" || lower == "paste that path" {
        return Some(ClipboardRecallIntent::Category("filepath".to_string()));
    }

    // 5. Semantic / Descriptive Query:
    // e.g. "paste the sql query I copied", "paste that github link", "paste the error message"
    let prefixes = [
        "paste the ", "paste that ", "paste the thing ", "paste the clip with ",
        "paste the text with ", "paste the message about ", "paste what ",
    ];

    for prefix in &prefixes {
        if lower.starts_with(prefix) {
            let remainder = &stripped[prefix.len()..].trim();
            if !remainder.is_empty() {
                let clean_desc = remainder
                    .trim_end_matches("i copied earlier")
                    .trim_end_matches("i copied")
                    .trim_end_matches("that i copied")
                    .trim();
                if !clean_desc.is_empty() {
                    return Some(ClipboardRecallIntent::SemanticDescription(clean_desc.to_string()));
                }
            }
        }
    }

    None
}

/// Uses Groq chat completions with active models (qwen/qwen3.8-27b, groq/compound-mini)
/// to fuzzy match the user's natural language voice recall query against recent clipboard clips.
pub async fn query_groq_clipboard_match(
    user_query: &str,
    recent_clips: &[ClipboardEntry],
) -> Result<Option<i64>, String> {
    if recent_clips.is_empty() {
        return Ok(None);
    }

    let api_key = CredentialsProvider::get_api_key()
        .ok_or_else(|| "Groq API key not found. Please enter your Groq API key in Settings -> Models.".to_string())?;

    let trimmed_key = api_key.trim();
    if trimmed_key.is_empty() {
        return Err("Groq API key is empty.".to_string());
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let system_prompt = "You are Rift Clipboard Voice Matcher.\n\
Given the user's voice recall command and a list of recent clipboard items, identify which item the user wants to paste.\n\
Return ONLY a valid JSON object: {\"matched_id\": <number or null>, \"confidence\": \"high\"|\"low\"|\"none\"}\n\
Do NOT output any markdown, explanations, or codeblocks. Just the raw JSON object.";

    let clip_summaries: Vec<Value> = recent_clips
        .iter()
        .take(15)
        .map(|c| {
            json!({
                "id": c.id,
                "category": c.category,
                "source_app": c.source_app.as_deref().unwrap_or("unknown"),
                "preview": c.preview
            })
        })
        .collect();

    let user_content = format!(
        "Voice Recall Command: \"{}\"\n\nRecent Clipboard Items:\n{}",
        user_query,
        serde_json::to_string_pretty(&clip_summaries).unwrap_or_default()
    );

    // Verified active Groq models in prioritized order
    let models_to_try = [
        "qwen/qwen3.8-27b",
        "groq/compound-mini",
        "openai/gpt-oss-20b",
    ];

    for model in &models_to_try {
        let req_body = json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user", "content": &user_content }
            ],
            "temperature": 0.0,
            "max_tokens": 100
        });

        let resp_res = client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .bearer_auth(trimmed_key)
            .header("User-Agent", "Rift-App/1.0")
            .json(&req_body)
            .send()
            .await;

        match resp_res {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(data) = resp.json::<Value>().await {
                    if let Some(content) = data["choices"][0]["message"]["content"].as_str() {
                        let cleaned = content.trim();
                        // Extract JSON substring if surrounded by markdown or quotes
                        let json_str = if let Some(start) = cleaned.find('{') {
                            if let Some(end) = cleaned.rfind('}') {
                                &cleaned[start..=end]
                            } else {
                                cleaned
                            }
                        } else {
                            cleaned
                        };

                        if let Ok(parsed) = serde_json::from_str::<Value>(json_str) {
                            let matched_id = parsed["matched_id"].as_i64();
                            let confidence = parsed["confidence"].as_str().unwrap_or("none");

                            if (confidence == "high" || confidence == "medium") && matched_id.is_some() {
                                let id = matched_id.unwrap();
                                // Validate that the matched_id belongs to the provided clips
                                if recent_clips.iter().any(|c| c.id == id) {
                                    return Ok(Some(id));
                                }
                            }
                        }
                    }
                }
            }
            Ok(resp) => {
                log::warn!("Groq model {} failed with status: {}", model, resp.status());
            }
            Err(e) => {
                log::warn!("Groq request error with {}: {}", model, e);
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recall_intent_relative_index() {
        assert_eq!(
            detect_clipboard_recall_intent("paste last clip"),
            Some(ClipboardRecallIntent::RelativeIndex(1))
        );
        assert_eq!(
            detect_clipboard_recall_intent("paste item 3"),
            Some(ClipboardRecallIntent::RelativeIndex(3))
        );
        assert_eq!(
            detect_clipboard_recall_intent("paste the thing I copied 2 items ago"),
            Some(ClipboardRecallIntent::RelativeIndex(2))
        );
    }

    #[test]
    fn test_recall_intent_category() {
        assert_eq!(
            detect_clipboard_recall_intent("paste the url"),
            Some(ClipboardRecallIntent::Category("url".to_string()))
        );
        assert_eq!(
            detect_clipboard_recall_intent("paste that email"),
            Some(ClipboardRecallIntent::Category("email".to_string()))
        );
    }
}
