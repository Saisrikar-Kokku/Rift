use regex::Regex;
use std::sync::LazyLock;

static RE_PARA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s*\b(?:new paragraph|next paragraph)\b\s*").unwrap()
});
static RE_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s*\b(?:new line|next line)\b\s*").unwrap()
});
static RE_TAB: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s*\b(?:tab key|insert tab)\b\s*").unwrap()
});

#[derive(Debug, PartialEq, Eq)]
pub enum VoiceCommandAction {
    Undo,
    FormattedText(String),
    None,
}

pub fn evaluate_voice_command(text: &str) -> VoiceCommandAction {
    let trimmed = text.trim();
    let lower = trimmed.to_lowercase();
    let stripped = lower.trim_end_matches(['.', ',', '!', '?', ';', ':']).trim();

    // 1. Voice Undo / Scratch commands
    if stripped == "scratch that"
        || stripped == "delete that"
        || stripped == "undo that"
        || stripped == "erase that"
        || stripped == "scratch this"
        || stripped == "undo last"
    {
        return VoiceCommandAction::Undo;
    }

    // 2. Bullet list formatting: "format as bullet list ...", "bullet list ..."
    if stripped.starts_with("bullet list:") || stripped.starts_with("bullet list") || stripped.starts_with("format as bullets") {
        let content = if let Some(idx) = lower.find("bullet list:") {
            &trimmed[idx + 12..]
        } else if let Some(idx) = lower.find("bullet list") {
            &trimmed[idx + 11..]
        } else if let Some(idx) = lower.find("format as bullets") {
            &trimmed[idx + 17..]
        } else {
            ""
        };

        let items: Vec<&str> = content
            .split(&['.', ';', '\n'][..])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        if !items.is_empty() {
            let bullets = items
                .iter()
                .map(|item| format!("- {}", item))
                .collect::<Vec<_>>()
                .join("\n");
            return VoiceCommandAction::FormattedText(bullets);
        }
    }

    // 3. Inline structure replacements: "new paragraph", "new line", "tab key"
    let mut modified = trimmed.to_string();
    let mut replaced = false;

    if RE_PARA.is_match(&modified) {
        modified = RE_PARA.replace_all(&modified, "\n\n").to_string();
        replaced = true;
    }

    if RE_LINE.is_match(&modified) {
        modified = RE_LINE.replace_all(&modified, "\n").to_string();
        replaced = true;
    }

    if RE_TAB.is_match(&modified) {
        modified = RE_TAB.replace_all(&modified, "\t").to_string();
        replaced = true;
    }

    if replaced {
        VoiceCommandAction::FormattedText(modified.trim().to_string())
    } else {
        VoiceCommandAction::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_undo_commands() {
        assert_eq!(evaluate_voice_command("scratch that"), VoiceCommandAction::Undo);
        assert_eq!(evaluate_voice_command("Delete that!"), VoiceCommandAction::Undo);
        assert_eq!(evaluate_voice_command("undo last."), VoiceCommandAction::Undo);
    }

    #[test]
    fn test_inline_replacements() {
        let res = evaluate_voice_command("Hello there new line how are you new paragraph all good");
        match res {
            VoiceCommandAction::FormattedText(t) => {
                assert_eq!(t, "Hello there\nhow are you\n\nall good");
            }
            _ => panic!("Expected FormattedText"),
        }
    }
}
