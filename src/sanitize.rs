use regex::Regex;
use std::sync::OnceLock;

const DEFAULT_PREVIEW_LEN: usize = 4000;

pub fn preview_len() -> usize {
    std::env::var("LOOPRS_PREVIEW_LEN")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_PREVIEW_LEN)
}

pub fn allow_raw_output() -> bool {
    std::env::var("LOOPRS_ALLOW_RAW_OUTPUT")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

pub fn strip_ansi(input: &str) -> String {
    // Remove common ANSI CSI sequences (e.g., ESC[...m)
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == 0x1b {
            // ESC
            if i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                // CSI: ESC [ ... <final>
                i += 2;
                while i < bytes.len() {
                    let b = bytes[i];
                    // Final byte typically in range 0x40..0x7E
                    if (0x40..=0x7e).contains(&b) {
                        i += 1;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
        }

        out.push(bytes[i]);
        i += 1;
    }

    String::from_utf8_lossy(&out).to_string()
}

pub fn sanitize_for_console(input: &str) -> String {
    if allow_raw_output() {
        return input.to_string();
    }

    let no_ansi = strip_ansi(input);
    redact(&no_ansi)
}

pub fn sanitize_preview_for_console(input: &str) -> String {
    let sanitized = sanitize_for_console(input);
    truncate_chars(&sanitized, preview_len())
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }

    let truncated: String = s.chars().take(max_chars).collect();
    format!(
        "{truncated}... [truncated {} chars]",
        s.chars().count().saturating_sub(max_chars)
    )
}

fn redact(input: &str) -> String {
    static PEM_RE: OnceLock<Regex> = OnceLock::new();
    static SK_RE: OnceLock<Regex> = OnceLock::new();
    static URL_CREDS_RE: OnceLock<Regex> = OnceLock::new();
    static KV_RE: OnceLock<Regex> = OnceLock::new();

    let pem_re = PEM_RE.get_or_init(|| {
        Regex::new(r"(?s)-----BEGIN (?:RSA )?PRIVATE KEY-----.*?-----END (?:RSA )?PRIVATE KEY-----")
            .expect("pem regex")
    });
    let sk_re = SK_RE.get_or_init(|| Regex::new(r"\bsk-[A-Za-z0-9_\-]{10,}\b").expect("sk regex"));
    let url_creds_re = URL_CREDS_RE.get_or_init(|| {
        Regex::new(r"(https?://)([^/\s:@]+):([^/\s@]+)@").expect("url creds regex")
    });
    let kv_re = KV_RE.get_or_init(|| {
        // key: value  OR  key=value  (JSON/YAML/env-like)
        Regex::new(
            r#"(?i)(api[_-]?key|authorization|access[_-]?token|token|secret|password)\s*([:=])\s*(["']?)([^\s"'\r\n,}]+)(["']?)"#,
        )
        .expect("kv regex")
    });

    let mut s = input.to_string();
    s = pem_re.replace_all(&s, "[REDACTED PRIVATE KEY]").to_string();
    s = url_creds_re.replace_all(&s, "$1$2:[REDACTED]@").to_string();
    s = sk_re.replace_all(&s, "sk-[REDACTED]").to_string();
    s = kv_re.replace_all(&s, "$1$2$3[REDACTED]$5").to_string();

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_sk_tokens() {
        let s = "token sk-abcdefghijklmnopqrstuvwxyz12345 end";
        let out = sanitize_for_console(s);
        assert!(out.contains("sk-[REDACTED]"));
        assert!(!out.contains("sk-abcdefghijklmnopqrstuvwxyz"));
    }

    #[test]
    fn redacts_key_value_pairs() {
        let s = "OPENAI_API_KEY=sk-abcdef1234567890\nauthorization: Bearer abcdef\n";
        let out = sanitize_for_console(s);
        assert!(out.to_lowercase().contains("api_key"));
        assert!(out.contains("[REDACTED]"));
    }

    #[test]
    fn redacts_pem_private_key() {
        let s = "-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----";
        let out = sanitize_for_console(s);
        assert_eq!(out, "[REDACTED PRIVATE KEY]");
    }

    #[test]
    fn redacts_url_credentials() {
        let s = "https://user:pass@example.com/path";
        let out = sanitize_for_console(s);
        assert_eq!(out, "https://user:[REDACTED]@example.com/path");
    }

    #[test]
    fn strips_ansi_sequences() {
        let s = "\u{1b}[31mred\u{1b}[0m";
        let out = strip_ansi(s);
        assert_eq!(out, "red");
    }
}
