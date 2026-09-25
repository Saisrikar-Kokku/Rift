/// Removes whitespace immediately preceding common punctuation marks.
pub fn clean_punctuation_spacing(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut i = 0;

    while i < n {
        let c = chars[i];
        if c.is_whitespace() && i + 1 < n {
            let next = chars[i + 1];
            if next == ',' || next == '.' || next == '!' || next == '?' || next == ':' || next == ';' {
                // Skip the whitespace before the punctuation mark
                i += 1;
                continue;
            }
        }
        result.push(c);
        i += 1;
    }
    result
}

/// Capitalizes the first character if it is a lowercase letter.
pub fn capitalize_first_letter(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            if first.is_lowercase() {
                first.to_uppercase().collect::<String>() + chars.as_str()
            } else {
                text.to_string()
            }
        }
    }
}

/// Collapses multiple consecutive spaces/tabs into a single space while preserving intentional newlines.
pub fn collapse_consecutive_spaces(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_spaces = false;
    for c in text.chars() {
        if c == ' ' || c == '\t' {
            if !in_spaces {
                result.push(' ');
                in_spaces = true;
            }
        } else {
            in_spaces = false;
            result.push(c);
        }
    }
    result
}

pub fn clean_transcript(raw_text: &str) -> String {
    if raw_text.is_empty() {
        return String::new();
    }
    let text = raw_text.trim();
    if text.is_empty() {
        return String::new();
    }

    // 1. Check and strip trailing ellipses / repeated dots
    let mut trailing_len = 0;
    let mut mark_count = 0;
    let mut has_ellipsis = false;

    for c in text.chars().rev() {
        if c.is_whitespace() || c == '.' || c == '…' || c == '。' {
            trailing_len += c.len_utf8();
            if c == '.' || c == '…' || c == '。' {
                mark_count += 1;
                if c == '…' {
                    has_ellipsis = true;
                }
            }
        } else {
            break;
        }
    }

    let base = if mark_count >= 2 || has_ellipsis {
        let clean_slice = &text[..text.len() - trailing_len];
        clean_slice.trim_end()
    } else {
        text
    };

    // 2. Clean punctuation spacing (remove erroneous spaces before commas/periods/question marks)
    let cleaned_spacing = clean_punctuation_spacing(base);

    // 3. Collapse accidental multiple spaces while preserving newlines
    let collapsed = collapse_consecutive_spaces(&cleaned_spacing);

    // 4. Capitalize first letter if lowercase
    capitalize_first_letter(&collapsed)
}

pub fn is_silence_or_hallucination(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return true;
    }

    // Filter pure punctuation strings
    if t.chars().all(|c| c.is_ascii_punctuation() || c.is_whitespace() || c == '…' || c == '。' || c == '、' || c == '—') {
        return true;
    }

    let lower = t.to_lowercase();
    let stripped = lower
        .trim_matches(|c: char| c.is_ascii_punctuation() || c.is_whitespace() || c == '…' || c == '。' || c == '、')
        .trim();

    if stripped.is_empty() {
        return true;
    }

    const HALLUCINATIONS: &[&str] = &[
        "you",
        "thank you",
        "thank you.",
        "thank you very much",
        "thank you so much",
        "thank you for watching",
        "thank you so much for watching",
        "thanks for watching",
        "thanks for watching!",
        "thanks for listening",
        "thank you for listening",
        "bye",
        "bye-bye",
        "bye bye",
        "goodbye",
        "see you next time",
        "see you in the next video",
        "music",
        "[music]",
        "(music)",
        "applause",
        "[applause]",
        "laughter",
        "[laughter]",
        "silence",
        "[silence]",
        "(silence)",
        "[blank_audio]",
        "amara.org",
        "subtitles by",
        "translated by",
        "subtitle by",
    ];

    for &h in HALLUCINATIONS {
        if stripped == h {
            return true;
        }
    }

    // Check repetitive single word loops (e.g. "oh oh oh", "you you you")
    let words: Vec<&str> = stripped.split_whitespace().collect();
    if words.len() >= 2 {
        let first = words[0];
        if words.iter().all(|&w| w == first) {
            return true;
        }

        // Check multi-word n-gram repetition loops (e.g. "thank you thank you", "thank you so much thank you so much")
        for k in 2..=4 {
            if words.len() >= k * 2 && words.len() % k == 0 {
                let pattern = &words[0..k];
                let is_loop = words.chunks(k).all(|chunk| chunk == pattern);
                if is_loop {
                    return true;
                }
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_transcript_ellipsis() {
        assert_eq!(clean_transcript("Hello world..."), "Hello world");
        assert_eq!(clean_transcript("Hello world…"), "Hello world");
        assert_eq!(clean_transcript("Hello world."), "Hello world.");
        assert_eq!(clean_transcript("  Hello there  "), "Hello there");
        assert_eq!(clean_transcript(""), "");
    }

    #[test]
    fn test_clean_punctuation_spacing() {
        assert_eq!(clean_punctuation_spacing("Hello , world !"), "Hello, world!");
        assert_eq!(clean_punctuation_spacing("Wait for it ..."), "Wait for it...");
        assert_eq!(clean_punctuation_spacing("Is it ready ? Yes ."), "Is it ready? Yes.");
    }

    #[test]
    fn test_collapse_consecutive_spaces() {
        assert_eq!(collapse_consecutive_spaces("Hello   world  test"), "Hello world test");
        assert_eq!(collapse_consecutive_spaces("Hello\n\nworld"), "Hello\n\nworld");
        assert_eq!(collapse_consecutive_spaces("Multiple    spaces\there"), "Multiple spaces here");
    }

    #[test]
    fn test_capitalize_first_letter() {
        assert_eq!(capitalize_first_letter("hello world"), "Hello world");
        assert_eq!(capitalize_first_letter("Hello world"), "Hello world");
        assert_eq!(capitalize_first_letter("123 numbers"), "123 numbers");
    }

    #[test]
    fn test_is_silence_or_hallucination() {
        assert!(is_silence_or_hallucination(""));
        assert!(is_silence_or_hallucination("   "));
        assert!(is_silence_or_hallucination("..."));
        assert!(is_silence_or_hallucination("Thank you."));
        assert!(is_silence_or_hallucination("Thank you for watching."));
        assert!(is_silence_or_hallucination("Thanks for watching!"));
        assert!(is_silence_or_hallucination("[Music]"));
        assert!(is_silence_or_hallucination("Oh Oh Oh"));
        assert!(is_silence_or_hallucination("Thank you thank you"));
        assert!(is_silence_or_hallucination("Thank you so much thank you so much"));
        assert!(is_silence_or_hallucination("Bye bye bye bye"));
        assert!(!is_silence_or_hallucination("Hello world"));
        assert!(!is_silence_or_hallucination("Can you please review this code?"));
    }
}

