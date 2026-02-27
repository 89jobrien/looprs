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
    if !cfg!(debug_assertions) {
        return false;
    }

    let explicitly_enabled = std::env::var("LOOPRS_ALLOW_RAW_OUTPUT")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));

    if !explicitly_enabled {
        return false;
    }

    let env = std::env::var("LOOPRS_ENV")
        .unwrap_or_default()
        .to_ascii_lowercase();

    !matches!(env.as_str(), "prod" | "production")
}

pub fn strip_ansi(input: &str) -> String {
    // Remove common ANSI CSI sequences (e.g., ESC[...m)
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == 0x1b {
            if i + 1 >= bytes.len() {
                break;
            }

            match bytes[i + 1] {
                b'[' => {
                    i += 2;
                    while i < bytes.len() {
                        let b = bytes[i];
                        i += 1;
                        if (0x40..=0x7e).contains(&b) {
                            break;
                        }
                    }
                    continue;
                }
                b']' => {
                    i += 2;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                    continue;
                }
                b'P' | b'_' | b'^' | b'X' => {
                    i += 2;
                    while i < bytes.len() {
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                    continue;
                }
                _ => {
                    i += 2;
                    continue;
                }
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
    static AUTH_RE: OnceLock<Regex> = OnceLock::new();
    static JWT_RE: OnceLock<Regex> = OnceLock::new();
    static PROVIDER_TOKEN_RE: OnceLock<Regex> = OnceLock::new();
    static KV_RE: OnceLock<Regex> = OnceLock::new();

    let pem_re = PEM_RE.get_or_init(|| {
        Regex::new(
            r"(?s)-----BEGIN (?:[A-Z0-9 ]*PRIVATE KEY|OPENSSH PRIVATE KEY|PGP PRIVATE KEY BLOCK)-----.*?-----END (?:[A-Z0-9 ]*PRIVATE KEY|OPENSSH PRIVATE KEY|PGP PRIVATE KEY BLOCK)-----",
        )
            .expect("pem regex")
    });
    let sk_re = SK_RE.get_or_init(|| Regex::new(r"\bsk-[A-Za-z0-9_\-]{10,}\b").expect("sk regex"));
    let url_creds_re = URL_CREDS_RE
        .get_or_init(|| Regex::new(r"(https?://)([^/\s@]+)@").expect("url creds regex"));
    let auth_re = AUTH_RE.get_or_init(|| {
        Regex::new(r"(?im)(\bauthorization\b\s*[:=]\s*)([^\r\n]+)").expect("auth regex")
    });
    let jwt_re = JWT_RE.get_or_init(|| {
        Regex::new(r"\beyJ[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\.[A-Za-z0-9_\-]{10,}\b")
            .expect("jwt regex")
    });
    let provider_token_re = PROVIDER_TOKEN_RE.get_or_init(|| {
        Regex::new(
            r"\b(?:ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|xox[baprs]-[A-Za-z0-9\-]{10,}|AKIA[0-9A-Z]{16})\b",
        )
        .expect("provider token regex")
    });
    let kv_re = KV_RE.get_or_init(|| {
        Regex::new(
            r#"(?im)(\b(?:api[_-]?key|access[_-]?token|refresh[_-]?token|id[_-]?token|token|secret|password|passwd|pwd|client[_-]?secret)\b\s*[:=]\s*)(?:\"[^\"\r\n]*\"|'[^'\r\n]*'|[^\r\n,}]+)"#,
        )
        .expect("kv regex")
    });

    let mut s = input.to_string();
    s = pem_re.replace_all(&s, "[REDACTED PRIVATE KEY]").to_string();
    s = url_creds_re.replace_all(&s, "$1[REDACTED]@").to_string();
    s = auth_re.replace_all(&s, "$1[REDACTED]").to_string();
    s = sk_re.replace_all(&s, "sk-[REDACTED]").to_string();
    s = jwt_re.replace_all(&s, "[REDACTED JWT]").to_string();
    s = provider_token_re
        .replace_all(&s, "[REDACTED TOKEN]")
        .to_string();
    s = kv_re.replace_all(&s, "$1[REDACTED]").to_string();

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
        assert_eq!(out, "https://[REDACTED]@example.com/path");
    }

    #[test]
    fn strips_ansi_sequences() {
        let s = "\u{1b}[31mred\u{1b}[0m";
        let out = strip_ansi(s);
        assert_eq!(out, "red");
    }

    #[test]
    fn strips_osc_sequences() {
        let s = "before\u{1b}]0;window-title\u{7}after";
        let out = strip_ansi(s);
        assert_eq!(out, "beforeafter");
    }

    #[test]
    fn redacts_authorization_header() {
        let s = "Authorization: Bearer super-secret-token";
        let out = sanitize_for_console(s);
        assert_eq!(out, "Authorization: [REDACTED]");
    }

    #[test]
    fn redacts_jwt_tokens() {
        let s = "jwt eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.abcdefghijklmno1234567890.pqrstuvwxyzABCDE1234567890";
        let out = sanitize_for_console(s);
        assert!(out.contains("[REDACTED JWT]"));
    }

    #[test]
    fn redacts_provider_tokens() {
        let s = "ghp_abcdefghijklmnopqrstuvwxyz123456 and AKIA1234567890ABCDEF";
        let out = sanitize_for_console(s);
        assert!(!out.contains("ghp_abcdefghijklmnopqrstuvwxyz123456"));
        assert!(!out.contains("AKIA1234567890ABCDEF"));
        assert!(out.contains("[REDACTED TOKEN]"));
    }
}
