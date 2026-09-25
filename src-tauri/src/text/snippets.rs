use crate::storage::snippets::VoiceSnippet;
use chrono::Local;

pub fn expand_tokens(template: &str) -> String {
    let now = Local::now();
    let date_str = now.format("%B %d, %Y").to_string(); // e.g. September 12, 2026
    let time_str = now.format("%I:%M %p").to_string();   // e.g. 07:30 PM

    let mut result = template.replace("{date}", &date_str);
    result = result.replace("{time}", &time_str);

    if result.contains("{clipboard}") {
        let clip = crate::text::injector::TextInjector::get_clipboard_text().unwrap_or_default();
        result = result.replace("{clipboard}", &clip);
    }

    result
}

pub fn match_and_expand(transcript: &str, snippets: &[VoiceSnippet]) -> Option<String> {
    let clean = transcript.trim().trim_end_matches(['.', ',', '!', '?']).to_lowercase();
    if clean.is_empty() {
        return None;
    }

    for s in snippets {
        if !s.is_active {
            continue;
        }
        let trigger = s.trigger_phrase.trim().to_lowercase();
        if trigger.is_empty() {
            continue;
        }

        if s.match_type == "prefix" {
            if clean.starts_with(&trigger) {
                let remainder = clean[trigger.len()..].trim();
                let expanded = expand_tokens(&s.expansion_text);
                if remainder.is_empty() {
                    return Some(expanded);
                } else {
                    return Some(format!("{}\n{}", expanded, remainder));
                }
            }
        } else {
            // exact match
            if clean == trigger {
                return Some(expand_tokens(&s.expansion_text));
            }
        }
    }

    None
}
