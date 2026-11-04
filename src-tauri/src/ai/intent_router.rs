use serde_json::{json, Value};
use crate::ambient::context::{AppDomain, WindowContext};
use crate::storage::credentials::CredentialsProvider;

#[derive(Debug, Clone, PartialEq)]
pub enum RoutedIntent {
    /// Mode A: Standard 0ms fast-path verbatim/enhanced dictation
    DirectDictation,
    /// Mode B: Rewrite/transform currently highlighted text in the active window
    SelectionTransform {
        instruction: String,
        selected_text: String,
    },
    /// Mode C: Generate code, regex, terminal command, email draft, or structured content
    GenerativeOutput {
        prompt: String,
    },
    /// Mode D: Save a background note to the Scratchpad without typing into the active app
    BackgroundNote {
        note_body: String,
    },
}

/// Strips any leading "Hey Rift" / "Hi Rift" wake-word prefix from a transcript
/// so hands-free wake-word utterances never type "Hey Rift" into the target app.
pub fn strip_wake_word_prefix(text: &str) -> String {
    let trimmed = text.trim();
    let lower = trimmed.to_lowercase();

    let prefixes = [
        "hey rift",
        "hey, rift",
        "hi rift",
        "hi, rift",
        "okay rift",
        "ok rift",
        "yo rift",
        "hey rithvik",
        "hey, rithvik",
        "hey ritvik",
        "hey rith",
        "hey rit",
        "hey rif",
        "hey ript",
        "hey drift",
        "hey lift",
        "hey wrist",
        "hey ref",
        "hey rick",
        "hey rich",
        "hey gift",
        "hey swift",
        "hair rift",
        "హే రిఫ్ట్",
        "హే రిత్విక్",
    ];

    for prefix in &prefixes {
        if lower.starts_with(prefix) {
            let rest = &trimmed[prefix.len()..];
            let cleaned = rest
                .trim_start_matches(|c: char| {
                    c == ',' || c == '.' || c == ':' || c == '-' || c == '!' || c == '?' || c.is_whitespace()
                })
                .trim();
            if !cleaned.is_empty() {
                // Capitalize first character of remaining sentence if it was lowercase ASCII
                let mut chars = cleaned.chars();
                if let Some(first) = chars.next() {
                    if first.is_ascii_lowercase() {
                        return format!("{}{}", first.to_uppercase(), chars.as_str());
                    }
                    return cleaned.to_string();
                }
            }
            return String::new();
        }
    }

    trimmed.to_string()
}

/// Checks if the spoken transcript is a background Scratchpad note intent (Mode D).
pub fn detect_background_note(transcript: &str) -> Option<String> {
    let trimmed = transcript.trim();
    let lower = trimmed.to_lowercase();

    let note_prefixes = [
        "note to self",
        "add to scratchpad",
        "save to scratchpad",
        "scratchpad note",
        "save a note",
        "take a note",
        "jot down",
        "jot this down",
    ];

    for prefix in &note_prefixes {
        if lower.starts_with(prefix) {
            let rest = &trimmed[prefix.len()..];
            let body = rest
                .trim_start_matches(|c: char| {
                    c == ':' || c == ',' || c == '-' || c == '.' || c.is_whitespace()
                })
                .trim();
            if !body.is_empty() {
                let mut chars = body.chars();
                if let Some(first) = chars.next() {
                    return Some(format!("{}{}", first.to_uppercase(), chars.as_str()));
                }
            }
        }
    }

    None
}

/// Fast 0ms heuristic check: returns true if the spoken transcript sounds like an instruction
/// to transform/rewrite/edit currently highlighted text (Mode B).
/// We run this check BEFORE probing the target window's selection via Ctrl+C so normal
/// dictation incurs 0.0ms overhead.
pub fn looks_like_selection_instruction(transcript: &str) -> bool {
    let lower = transcript
        .trim()
        .trim_end_matches(|c: char| c == '.' || c == '!' || c == '?')
        .to_lowercase();

    let word_count = lower.split_whitespace().count();
    if word_count == 0 || word_count > 22 {
        return false;
    }

    let exact_or_prefix_triggers = [
        "make this ",
        "make it ",
        "rewrite this",
        "rephrase this",
        "summarize this",
        "summarise this",
        "fix the grammar",
        "fix grammar",
        "fix spelling",
        "correct this",
        "polish this",
        "improve this",
        "shorten this",
        "expand this",
        "elaborate on this",
        "translate this ",
        "translate to ",
        "convert this to ",
        "convert to ",
        "turn this into ",
        "format this as ",
        "format as ",
        "bulletize this",
        "convert to bullet",
        "refactor this",
        "optimize this",
        "add comments to this",
        "add types to this",
        "add docstrings to this",
        "explain this",
    ];

    for trigger in &exact_or_prefix_triggers {
        if lower.starts_with(trigger) || lower == trigger.trim() {
            return true;
        }
    }

    false
}

/// Checks if the spoken transcript is an explicit generative command (Mode C).
pub fn detect_generative_prompt(transcript: &str) -> Option<String> {
    let trimmed = transcript.trim();
    let lower = trimmed.to_lowercase();

    let gen_prefixes = [
        "ask rift to ",
        "ask rift ",
        "rift generate ",
        "rift write ",
        "generate a ",
        "generate an ",
        "generate the ",
        "generate code for ",
        "generate ",
        "write me a ",
        "write me an ",
        "write a ",
        "write an ",
        "write code for ",
        "write the code for ",
        "draft a ",
        "draft an ",
        "draft a reply ",
        "draft an email ",
        "give me the code for ",
        "give me code for ",
        "give me a regex for ",
        "give me the regex for ",
        "give me the command for ",
        "give me a command for ",
        "give me a terminal command ",
        "give me a powershell command ",
        "give me a bash command ",
        "give me a sql query ",
        "give me a formula for ",
        "create a ",
        "create an ",
    ];

    for prefix in &gen_prefixes {
        if lower.starts_with(prefix) {
            // Keep the full instruction so the LLM has complete context (unless it starts with "ask rift")
            let clean_prompt = if lower.starts_with("ask rift to ") {
                trimmed["ask rift to ".len()..].trim().to_string()
            } else if lower.starts_with("ask rift ") {
                trimmed["ask rift ".len()..].trim().to_string()
            } else if lower.starts_with("rift ") {
                trimmed["rift ".len()..].trim().to_string()
            } else {
                trimmed.to_string()
            };

            if clean_prompt.split_whitespace().count() >= 2 {
                return Some(clean_prompt);
            }
        }
    }

    None
}

/// Executes Mode B (Inline Selection Magic Edit) using Groq Llama-3.3-70b-versatile (with OpenRouter fallback).
pub async fn execute_selection_transform(
    instruction: &str,
    selected_text: &str,
    window_ctx: &WindowContext,
) -> Result<String, String> {
    let domain_hint = match window_ctx.domain {
        AppDomain::Coding => "The user is inside a code editor (IDE). If transforming code, output ONLY valid code without markdown triple-backtick wrappers unless explicitly asked.",
        AppDomain::Terminal => "The user is inside a terminal/shell. Output ONLY the raw command or text without markdown wrappers.",
        AppDomain::Messaging => "The user is in a chat/messaging app. Keep the tone natural, concise, and ready to send.",
        AppDomain::Document => "The user is editing a document or email. Maintain clean formatting and structure.",
        _ => "Output ONLY the transformed text ready to replace the user's selection in-place.",
    };

    let system_prompt = format!(
        "You are Rift Voice Intent Router (Inline Selection Editor).\n\
         The user has highlighted text on their screen and spoken a voice instruction to transform or edit it.\n\
         {}\n\
         CRITICAL RULES:\n\
         1. Output ONLY the final transformed/rewritten text that will directly replace the highlighted selection.\n\
         2. NEVER include conversational filler like 'Sure, here is the rewritten text:' or surrounding quotation marks.\n\
         3. Follow the user's spoken instruction faithfully while preserving key facts/logic from the original selection.",
        domain_hint
    );

    let user_message = format!(
        "HIGHLIGHTED SELECTION:\n\"\"\"\n{}\n\"\"\"\n\nSPOKEN INSTRUCTION:\n{}",
        selected_text.trim(),
        instruction.trim()
    );

    run_fast_llm(&system_prompt, &user_message).await
}

/// Executes Mode C (Generative Write / Code / Command Output) using Groq Llama-3.3-70b-versatile (with OpenRouter fallback).
pub async fn execute_generative_output(
    prompt: &str,
    window_ctx: &WindowContext,
) -> Result<String, String> {
    let domain_hint = match window_ctx.domain {
        AppDomain::Coding => {
            "The user is currently focused inside a Code Editor / IDE. \
             When generating code, functions, or regex, output ONLY the raw code itself (NO ```markdown``` fences, NO explanations before or after) so it can be pasted directly at their cursor."
        }
        AppDomain::Terminal => {
            "The user is currently focused inside a Terminal / PowerShell / Command Prompt. \
             Output ONLY the single-line or multi-line executable shell command (NO ```markdown``` fences, NO explanations)."
        }
        AppDomain::Messaging => {
            "The user is currently in a Messaging / Chat app. \
             Output ONLY the ready-to-send message body without conversational preamble."
        }
        AppDomain::Document => {
            "The user is currently in a Document / Email / Notes app. \
             Output ONLY the requested draft, paragraph, list, or content without conversational preamble."
        }
        _ => {
            "If the user asks for code, regex, a formula, or a terminal command, output ONLY the raw code/command without markdown fences. \
             If the user asks to draft an email, message, or paragraph, output ONLY the ready-to-use text without conversational preamble."
        }
    };

    let system_prompt = format!(
        "You are Rift Voice Intent Router (Direct Cursor Generator).\n\
         The user spoke a voice command asking you to generate content directly into their active application ({}).\n\
         {}\n\
         CRITICAL RULES:\n\
         1. Output ONLY the final generated text or code that should be typed at the cursor.\n\
         2. NEVER include conversational intro/outro like 'Here is...' or 'Sure!'.\n\
         3. Do NOT wrap code or commands in ```backticks```.",
        if window_ctx.process_name.is_empty() {
            "Active Window"
        } else {
            &window_ctx.process_name
        },
        domain_hint
    );

    run_fast_llm(&system_prompt, prompt.trim()).await
}

async fn run_fast_llm(system_prompt: &str, user_message: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    // Tier 1: Try Groq LPU (llama-3.3-70b-versatile -> llama-3.1-8b-instant) for ultra-low latency
    if let Some(groq_key) = CredentialsProvider::get_api_key() {
        let gkey = groq_key.trim();
        if !gkey.is_empty() {
            for model in &["llama-3.3-70b-versatile", "llama-3.1-8b-instant"] {
                let req_body = json!({
                    "model": model,
                    "messages": [
                        { "role": "system", "content": system_prompt },
                        { "role": "user", "content": user_message }
                    ],
                    "temperature": 0.2,
                    "max_tokens": 2048
                });

                if let Ok(resp) = client
                    .post("https://api.groq.com/openai/v1/chat/completions")
                    .bearer_auth(gkey)
                    .header("User-Agent", "Rift-App/1.0")
                    .json(&req_body)
                    .send()
                    .await
                {
                    if resp.status().is_success() {
                        if let Ok(data) = resp.json::<Value>().await {
                            if let Some(content) = data["choices"][0]["message"]["content"].as_str() {
                                let cleaned = strip_outer_code_fences(content.trim());
                                if !cleaned.is_empty() {
                                    return Ok(cleaned);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Tier 2: Fallback to OpenRouter (google/gemini-2.5-flash or mistralai/mistral-small-24b-instruct-2501)
    if let Some(or_key) = CredentialsProvider::get_openrouter_api_key() {
        let okey = or_key.trim();
        if !okey.is_empty() {
            let req_body = json!({
                "model": "google/gemini-2.5-flash",
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": user_message }
                ],
                "temperature": 0.2,
                "max_tokens": 2048
            });

            if let Ok(resp) = client
                .post("https://openrouter.ai/api/v1/chat/completions")
                .bearer_auth(okey)
                .header("HTTP-Referer", "https://rift.app")
                .header("X-Title", "Rift Voice Assistant")
                .json(&req_body)
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(data) = resp.json::<Value>().await {
                        if let Some(content) = data["choices"][0]["message"]["content"].as_str() {
                            let cleaned = strip_outer_code_fences(content.trim());
                            if !cleaned.is_empty() {
                                return Ok(cleaned);
                            }
                        }
                    }
                }
            }
        }
    }

    Err("No AI provider key available for Voice Intent Router (configure Groq or OpenRouter key in Settings).".to_string())
}

/// Removes outer ```lang ... ``` markdown code fences if the LLM included them despite instructions.
fn strip_outer_code_fences(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.starts_with("```") && trimmed.ends_with("```") && trimmed.len() > 6 {
        let without_start = &trimmed[3..];
        if let Some(newline_idx) = without_start.find('\n') {
            let inner = &without_start[newline_idx + 1..without_start.len() - 3];
            return inner.trim().to_string();
        }
    }
    trimmed.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_wake_word_prefix() {
        assert_eq!(
            strip_wake_word_prefix("Hey Rift, I will be there in 5 minutes."),
            "I will be there in 5 minutes."
        );
        assert_eq!(
            strip_wake_word_prefix("hey rift make this more professional"),
            "Make this more professional"
        );
        assert_eq!(
            strip_wake_word_prefix("Normal sentence without wake word."),
            "Normal sentence without wake word."
        );
    }

    #[test]
    fn test_detect_background_note() {
        assert_eq!(
            detect_background_note("Note to self: buy almond milk after gym"),
            Some("Buy almond milk after gym".to_string())
        );
        assert_eq!(
            detect_background_note("Add to scratchpad check the staging database logs"),
            Some("Check the staging database logs".to_string())
        );
        assert_eq!(detect_background_note("Hello how are you"), None);
    }

    #[test]
    fn test_looks_like_selection_instruction() {
        assert!(looks_like_selection_instruction("Make this more professional."));
        assert!(looks_like_selection_instruction("Summarize this into 3 bullet points"));
        assert!(looks_like_selection_instruction("Translate this to Spanish"));
        assert!(!looks_like_selection_instruction(
            "I think we should schedule the meeting for tomorrow afternoon at 3 PM."
        ));
    }

    #[test]
    fn test_detect_generative_prompt() {
        assert!(detect_generative_prompt("Write a Python function to parse a CSV file").is_some());
        assert!(detect_generative_prompt("Give me the regex for validating an email address").is_some());
        assert!(detect_generative_prompt("Draft a polite reply declining the meeting").is_some());
        assert!(detect_generative_prompt("Just typing a normal message to my friend").is_none());
    }
}
