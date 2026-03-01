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

fn normalize_overlap_token(token: &str) -> String {
    token
        .trim_matches(|ch: char| !ch.is_ascii_alphanumeric())
        .to_ascii_lowercase()
}

pub fn merge_transcript_segments(current: &str, incoming: &str) -> String {
    let normalized_current = current.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized_incoming = incoming.split_whitespace().collect::<Vec<_>>().join(" ");

    if normalized_current.is_empty() {
        return normalized_incoming;
    }
    if normalized_incoming.is_empty() {
        return normalized_current;
    }

    let current_lower = normalized_current.to_ascii_lowercase();
    let incoming_lower = normalized_incoming.to_ascii_lowercase();

    if current_lower == incoming_lower || current_lower.ends_with(&incoming_lower) {
        return normalized_current;
    }
    if incoming_lower.starts_with(&current_lower) {
        return normalized_incoming;
    }

    let current_tokens = normalized_current.split_whitespace().collect::<Vec<_>>();
    let incoming_tokens = normalized_incoming.split_whitespace().collect::<Vec<_>>();
    let max_overlap = 6usize.min(current_tokens.len()).min(incoming_tokens.len());

    for overlap in (1..=max_overlap).rev() {
        let mut matches = true;

        for index in 0..overlap {
            let current_index = current_tokens.len() - overlap + index;
            let left = normalize_overlap_token(current_tokens[current_index]);
            let right = normalize_overlap_token(incoming_tokens[index]);
            if left.is_empty() || right.is_empty() || left != right {
                matches = false;
                break;
            }
        }

        if matches {
            let remainder = incoming_tokens[overlap..].join(" ");
            if remainder.is_empty() {
                return normalized_current;
            }
            return format!("{normalized_current} {remainder}");
        }
    }

    format!("{normalized_current} {normalized_incoming}")
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
        assert!(is_duplicate_transcript(
            Some("Hello world."),
            "hello world."
        ));
        assert!(!is_duplicate_transcript(Some("Hello world."), "different"));
    }

    #[test]
    fn merge_segments_appends_continuous_speech() {
        let merged =
            merge_transcript_segments("At 7:45 a.m. I walked", "three blocks to Maple Street.");
        assert_eq!(
            merged,
            "At 7:45 a.m. I walked three blocks to Maple Street."
        );
    }

    #[test]
    fn merge_segments_deduplicates_boundary_overlap() {
        let merged = merge_transcript_segments(
            "Our team discussed budget numbers including",
            "including $14,250 for hardware.",
        );
        assert_eq!(
            merged,
            "Our team discussed budget numbers including $14,250 for hardware."
        );
    }
}
