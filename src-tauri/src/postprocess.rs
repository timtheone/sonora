pub fn normalize_transcript(input: &str) -> String {
    let collapsed = input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string();

    if collapsed.is_empty() {
        return String::new();
    }

    let mut chars = collapsed.chars();
    let first = chars
        .next()
        .map(|ch| ch.to_uppercase().to_string())
        .unwrap_or_default();
    let rest = chars.as_str();
    let mut sentence = format!("{first}{rest}");

    if !sentence.ends_with('.') && !sentence.ends_with('!') && !sentence.ends_with('?') {
        sentence.push('.');
    }

    sentence
}

pub fn is_duplicate_transcript(previous: Option<&str>, current: &str) -> bool {
    let normalized_current = current.trim().to_lowercase();
    if normalized_current.is_empty() {
        return true;
    }

    previous
        .map(|value| value.trim().to_lowercase() == normalized_current)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_whitespace_and_punctuation() {
        let output = normalize_transcript("   hello   world   ");
        assert_eq!(output, "Hello world.");
    }

    #[test]
    fn keeps_existing_terminal_punctuation() {
        assert_eq!(normalize_transcript("what now?"), "What now?");
    }

    #[test]
    fn duplicate_detection_ignores_case() {
        assert!(is_duplicate_transcript(Some("Hello world."), "hello world."));
        assert!(!is_duplicate_transcript(Some("Hello world."), "different"));
    }
}
