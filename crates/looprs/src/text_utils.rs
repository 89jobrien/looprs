/// Remove control characters (except LF/CR/TAB) and cap content length.
pub fn sanitize_and_truncate(input: &str, max_len: usize) -> String {
    if max_len == 0 {
        return String::new();
    }

    let sanitized: String = input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t')
        .collect();

    if sanitized.chars().count() <= max_len {
        return sanitized;
    }

    let mut out = sanitized
        .chars()
        .take(max_len.saturating_sub(1))
        .collect::<String>();
    out.push('\u{2026}');
    out
}

#[cfg(test)]
mod tests {
    use super::sanitize_and_truncate;

    #[test]
    fn keeps_printable_content_when_under_limit() {
        assert_eq!(sanitize_and_truncate("hello", 10), "hello");
    }

    #[test]
    fn strips_disallowed_control_chars() {
        assert_eq!(sanitize_and_truncate("a\u{0}b", 10), "ab");
    }

    #[test]
    fn truncates_and_appends_ellipsis() {
        let value = sanitize_and_truncate("abcdef", 5);
        assert_eq!(value.chars().count(), 5);
        assert_eq!(value.chars().last(), Some('\u{2026}'));
    }

    #[test]
    fn zero_length_cap_returns_empty_output() {
        assert_eq!(sanitize_and_truncate("content", 0), "");
    }

    #[test]
    fn one_character_cap_returns_only_ellipsis_when_truncated() {
        assert_eq!(sanitize_and_truncate("ab", 1), "\u{2026}");
    }

    #[test]
    fn multibyte_content_is_truncated_on_character_boundaries() {
        assert_eq!(sanitize_and_truncate("\u{e9}clair", 3), "\u{e9}c\u{2026}");
    }
}
