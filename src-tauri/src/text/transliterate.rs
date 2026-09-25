use reqwest::Client;
use serde_json::Value;
use std::time::Duration;

/// Checks if a string contains any Telugu Unicode characters (U+0C00 - U+0C7F).
pub fn contains_telugu(text: &str) -> bool {
    text.chars().any(|c| ('\u{0C00}'..='\u{0C7F}').contains(&c))
}

/// Zero-latency, deterministic Unicode-to-Roman phonetic transliteration for Telugu.
/// Used for instant mode and as a resilient local fallback if cloud LLMs are unreachable.
pub fn transliterate_telugu_rule_based(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(chars.len() * 2);
    let n = chars.len();
    let mut i = 0;

    while i < n {
        let c = chars[i];

        // 1. Independent Vowels
        if let Some(v) = match c {
            '\u{0C05}' => Some("a"),
            '\u{0C06}' => Some("aa"),
            '\u{0C07}' => Some("i"),
            '\u{0C08}' => Some("ee"),
            '\u{0C09}' => Some("u"),
            '\u{0C0A}' => Some("oo"),
            '\u{0C0B}' => Some("ru"),
            '\u{0C60}' => Some("roo"),
            '\u{0C0C}' => Some("lu"),
            '\u{0C61}' => Some("loo"),
            '\u{0C0E}' => Some("e"),
            '\u{0C0F}' => Some("e"),
            '\u{0C10}' => Some("ai"),
            '\u{0C12}' => Some("o"),
            '\u{0C13}' => Some("o"),
            '\u{0C14}' => Some("au"),
            _ => None,
        } {
            out.push_str(v);
            i += 1;
            continue;
        }

        // 2. Consonants
        let cons = match c {
            '\u{0C15}' => Some("k"),
            '\u{0C16}' => Some("kh"),
            '\u{0C17}' => Some("g"),
            '\u{0C18}' => Some("gh"),
            '\u{0C19}' => Some("ng"),
            '\u{0C1A}' => Some("ch"),
            '\u{0C1B}' => Some("chh"),
            '\u{0C1C}' => Some("j"),
            '\u{0C1D}' => Some("jh"),
            '\u{0C1E}' => Some("ny"),
            '\u{0C1F}' => Some("t"),
            '\u{0C20}' => Some("th"),
            '\u{0C21}' => Some("d"),
            '\u{0C22}' => Some("dh"),
            '\u{0C23}' => Some("n"),
            '\u{0C24}' => Some("th"),
            '\u{0C25}' => Some("th"),
            '\u{0C26}' => Some("d"),
            '\u{0C27}' => Some("dh"),
            '\u{0C28}' => Some("n"),
            '\u{0C2A}' => Some("p"),
            '\u{0C2B}' => Some("ph"),
            '\u{0C2C}' => Some("b"),
            '\u{0C2D}' => Some("bh"),
            '\u{0C2E}' => Some("m"),
            '\u{0C2F}' => Some("y"),
            '\u{0C30}' => Some("r"),
            '\u{0C31}' => Some("r"),
            '\u{0C32}' => Some("l"),
            '\u{0C33}' => Some("l"),
            '\u{0C35}' => Some("v"),
            '\u{0C36}' => Some("sh"),
            '\u{0C37}' => Some("sh"),
            '\u{0C38}' => Some("s"),
            '\u{0C39}' => Some("h"),
            _ => None,
        };

        if let Some(c_str) = cons {
            if i + 1 < n {
                let next_c = chars[i + 1];
                // Virama (halant U+0C4D): suppresses inherent 'a'
                if next_c == '\u{0C4D}' {
                    out.push_str(c_str);
                    i += 2;
                    continue;
                }
                // Matra (dependent vowel sign): replaces inherent 'a'
                if let Some(matra) = match next_c {
                    '\u{0C3E}' => Some("aa"),
                    '\u{0C3F}' => Some("i"),
                    '\u{0C40}' => Some("ee"),
                    '\u{0C41}' => Some("u"),
                    '\u{0C42}' => Some("oo"),
                    '\u{0C43}' => Some("ru"),
                    '\u{0C44}' => Some("roo"),
                    '\u{0C46}' => Some("e"),
                    '\u{0C47}' => Some("e"),
                    '\u{0C48}' => Some("ai"),
                    '\u{0C4A}' => Some("o"),
                    '\u{0C4B}' => Some("o"),
                    '\u{0C4C}' => Some("au"),
                    _ => None,
                } {
                    out.push_str(c_str);
                    out.push_str(matra);
                    i += 2;
                    continue;
                }
            }
            // Inherent 'a' vowel
            out.push_str(c_str);
            out.push('a');
            i += 1;
            continue;
        }

        // 3. Anusvara (U+0C02 ం) & Visarga (U+0C03 ః)
        if c == '\u{0C02}' {
            // Contextual nasalization: 'm' before p, b, m, or at word end / punctuation; otherwise 'n'
            let is_m = if i + 1 >= n {
                true
            } else {
                let next = chars[i + 1];
                next.is_whitespace()
                    || next.is_ascii_punctuation()
                    || matches!(next, '\u{0C2A}' | '\u{0C2B}' | '\u{0C2C}' | '\u{0C2D}' | '\u{0C2E}')
            };
            if is_m {
                out.push('m');
            } else {
                out.push('n');
            }
            i += 1;
            continue;
        }

        if c == '\u{0C03}' {
            out.push('h');
            i += 1;
            continue;
        }

        // 4. All other characters (English letters, numbers, spaces, emojis, punctuation) pass through
        out.push(c);
        i += 1;
    }

    out
}

const TRANSLITERATION_SYSTEM_PROMPT: &str = "\
You are a specialized Telugu-to-Romanized (Tenglish) transliteration engine.
Convert spoken Telugu text into natural Romanized Telugu (Tenglish) suitable for WhatsApp and direct messaging.
Strict rules:
1. Convert Telugu script phonetically into English letters (e.g. 'ఏం చేస్తున్నావు?' -> 'Em chesthunnav?').
2. Keep English loanwords in standard English spelling (e.g., 'office', 'lunch', 'WhatsApp', 'car', 'message', 'call', 'time').
3. Use colloquial, chat-friendly conversational spelling (e.g., 'bagunnanu', 'ekkada unnav', 'repu kaluddam').
4. Preserve punctuation, numbers, and capitalization.
5. CRITICAL: Output ONLY the transliterated text. Never explain, never add quotes, never translate into English meaning ('What are you doing?'), and NEVER converse.";

/// Transliterates Telugu text to natural Tenglish using Groq (Free · Ultra-fast).
pub async fn transliterate_telugu_groq(
    client: &Client,
    text: &str,
    api_key: &str,
) -> Result<String, String> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err("Empty Groq API key".to_string());
    }

    let body = serde_json::json!({
        "model": "qwen/qwen3.8-27b",
        "messages": [
            { "role": "system", "content": TRANSLITERATION_SYSTEM_PROMPT },
            { "role": "user", "content": text }
        ],
        "temperature": 0.1,
        "max_tokens": 120
    });

    log::info!("[Tenglish Groq] Requesting transliteration for: '{}'", text);
    let res = tokio::time::timeout(
        Duration::from_millis(6000),
        client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", key))
            .header("User-Agent", "Rift/1.0")
            .header("Content-Type", "application/json")
            .json(&body)
            .send(),
    )
    .await
    .map_err(|_| "Groq transliteration request timed out".to_string())?
    .map_err(|e| format!("Groq network error: {}", e))?;

    if !res.status().is_success() {
        let status = res.status();
        let err_text = res.text().await.unwrap_or_default();
        log::warn!("[Tenglish Groq] API error ({}): {}", status, err_text);
        return Err(format!("Groq API error ({}): {}", status, err_text));
    }

    let json: Value = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse Groq response: {}", e))?;

    if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
        let cleaned = clean_llm_output(content);
        if !cleaned.is_empty() {
            log::info!("[Tenglish Groq] Result: '{}'", cleaned);
            return Ok(cleaned);
        }
    }

    log::warn!("[Tenglish Groq] Invalid or empty response");
    Err("Invalid or empty response from Groq".to_string())
}

/// Transliterates Telugu text to natural Tenglish using OpenRouter (Studio/Premium Grade).
pub async fn transliterate_telugu_openrouter(
    client: &Client,
    text: &str,
    api_key: &str,
    model: &str,
) -> Result<String, String> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err("Empty OpenRouter API key".to_string());
    }

    let target_model = if model.is_empty() || model == "auto" {
        "google/gemini-2.5-flash"
    } else {
        model
    };

    let body = serde_json::json!({
        "model": target_model,
        "messages": [
            { "role": "system", "content": TRANSLITERATION_SYSTEM_PROMPT },
            { "role": "user", "content": text }
        ],
        "temperature": 0.1,
        "max_tokens": 120
    });

    log::info!("[Tenglish OpenRouter] Requesting transliteration with '{}' for: '{}'", target_model, text);
    let res = tokio::time::timeout(
        Duration::from_millis(8000),
        client
            .post("https://openrouter.ai/api/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", key))
            .header("HTTP-Referer", "https://rift.ai")
            .header("X-Title", "Rift Voice Dictation")
            .header("Content-Type", "application/json")
            .json(&body)
            .send(),
    )
    .await
    .map_err(|_| "OpenRouter transliteration request timed out".to_string())?
    .map_err(|e| format!("OpenRouter network error: {}", e))?;

    if !res.status().is_success() {
        let status = res.status();
        let err_text = res.text().await.unwrap_or_default();
        log::warn!("[Tenglish OpenRouter] API error ({}): {}", status, err_text);
        return Err(format!("OpenRouter API error ({}): {}", status, err_text));
    }

    let json: Value = res
        .json()
        .await
        .map_err(|e| format!("Failed to parse OpenRouter response: {}", e))?;

    if let Some(content) = json["choices"][0]["message"]["content"].as_str() {
        let cleaned = clean_llm_output(content);
        if !cleaned.is_empty() {
            log::info!("[Tenglish OpenRouter] Result: '{}'", cleaned);
            return Ok(cleaned);
        }
    }

    log::warn!("[Tenglish OpenRouter] Invalid or empty response");
    Err("Invalid or empty response from OpenRouter".to_string())
}

/// Removes enclosing quotes, markdown code fences, thinking tags, or extraneous artifacts from LLM outputs.
pub fn clean_llm_output(raw: &str) -> String {
    let mut s = raw.trim().to_string();

    // Strip reasoning <think>...</think> tags if present
    if let Some(start) = s.find("<think>") {
        if let Some(end) = s.find("</think>") {
            s = format!("{}{}", &s[..start], &s[end + 8..]);
        }
    }

    // Strip markdown code fences ```...```
    if s.starts_with("```") && s.ends_with("```") {
        let lines: Vec<&str> = s.lines().collect();
        if lines.len() >= 2 {
            s = lines[1..lines.len() - 1].join("\n").trim().to_string();
        }
    }

    // Strip wrapping single or double quotes
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        if s.len() >= 2 {
            s = s[1..s.len() - 1].trim().to_string();
        }
    }

    // Strip optional "Tenglish:" or "Romanized:" prefix if accidentally generated
    for prefix in &["Tenglish:", "Romanized:", "Romanised:", "Output:"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.trim().to_string();
        }
    }

    s.trim().to_string()
}

/// Unified entry point for Telugu transliteration with strict provider honoring and smart cascading.
/// Returns: (transliterated_text, provider_used)
/// provider_used is one of: "groq", "openrouter", "offline", "passthrough"
pub async fn transliterate_telugu(
    client: &Client,
    text: &str,
    or_key: Option<&str>,
    _groq_key: Option<&str>,
    _engine_tier: &str,
    _stt_provider_used: Option<&str>,
) -> (String, &'static str) {
    if !contains_telugu(text) {
        return (text.to_string(), "passthrough");
    }

    // Setup 1: OpenRouter Google Gemini 2.5 Flash (Verified Studio Grade · Colloquial WhatsApp Romanization)
    if let Some(okey) = or_key {
        if !okey.trim().is_empty() {
            if let Ok(res) =
                transliterate_telugu_openrouter(client, text, okey, "google/gemini-2.5-flash").await
            {
                return (res, "openrouter");
            }
        }
    }

    // Resilient offline fallback: Instant Unicode-to-Latin rule-based
    (transliterate_telugu_rule_based(text), "offline")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contains_telugu() {
        assert!(contains_telugu("ఏం చేస్తున్నావు?"));
        assert!(contains_telugu("Hello ఏం"));
        assert!(!contains_telugu("Hello how are you?"));
        assert!(!contains_telugu("12345 !@#$"));
    }

    #[test]
    fn test_rule_based_transliteration() {
        let res = transliterate_telugu_rule_based("ఏం చేస్తున్నావు?");
        assert_eq!(res, "em chesthunnaavu?");

        let res2 = transliterate_telugu_rule_based("నేను బాగున్నాను");
        assert_eq!(res2, "nenu baagunnaanu");

        let res3 = transliterate_telugu_rule_based("ఇవాళ లంచ్ ఏం తిందాం?");
        assert!(res3.contains("em"));
        assert!(res3.contains("thindaam"));
    }

    #[test]
    fn test_clean_llm_output() {
        assert_eq!(clean_llm_output("\"Em chesthunnav?\""), "Em chesthunnav?");
        assert_eq!(clean_llm_output("```\nEm chesthunnav?\n```"), "Em chesthunnav?");
        assert_eq!(clean_llm_output("Tenglish: Em chesthunnav?"), "Em chesthunnav?");
    }
}
