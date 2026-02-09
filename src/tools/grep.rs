use super::error::ToolError;
use super::ToolArgs;
use super::ToolContext;
use regex::Regex;
use serde_json::Value;
use std::fs;
use std::process::Command;

use super::availability;
use crate::config::MAX_GREP_HITS;

/// Try to use ripgrep (rg) if available, fall back to pure regex implementation
pub(super) fn tool_grep(args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
    let args = ToolArgs::new(args);
    let pat_str = args.get_str("pat")?;
    let path_prefix = args.get_str_optional("path")?.unwrap_or(".");

    let base = ctx.resolve_path(path_prefix)?;

    // Try rg first if available
    if availability::is_rg_available() {
        if let Ok(result) = try_rg(pat_str, &base) {
            return Ok(result);
        }
    }

    // Fall back to pure Rust implementation
    grep_fallback(pat_str, &base)
}

/// Try to use ripgrep for searching
fn try_rg(pattern: &str, path: &std::path::Path) -> Result<String, ToolError> {
    let output = Command::new("rg")
        .arg("--max-count")
        .arg(MAX_GREP_HITS.to_string())
        .arg("--line-number")
        .arg("--no-heading")
        .arg("--color")
        .arg("never")
        .arg(pattern)
        .arg(path)
        .output()
        .map_err(|_| ToolError::MissingParameter("rg execution failed".to_string()))?;

    if !output.status.success() && output.status.code() != Some(1) {
        return Err(ToolError::CommandFailed(format!(
            "rg error: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout.lines().collect();

    if lines.is_empty() {
        Ok("none".to_string())
    } else {
        Ok(lines.join("\n"))
    }
}

/// Pure Rust fallback using regex
fn grep_fallback(pat_str: &str, base: &std::path::Path) -> Result<String, ToolError> {
    let re = Regex::new(pat_str)?;
    let glob_pattern = base.join("**/*");
    let pattern_str = glob_pattern
        .to_str()
        .ok_or_else(|| ToolError::InvalidPath(base.display().to_string()))?;

    let mut hits = Vec::new();

    for entry in glob::glob(pattern_str)?.filter_map(Result::ok) {
        if !entry.is_file() {
            continue;
        }

        let Ok(content) = fs::read_to_string(&entry) else {
            continue;
        };

        for (i, line) in content.lines().enumerate() {
            if re.is_match(line) {
                hits.push(format!("{}:{}: {}", entry.display(), i + 1, line.trim()));
                if hits.len() >= MAX_GREP_HITS {
                    break;
                }
            }
        }

        if hits.len() >= MAX_GREP_HITS {
            break;
        }
    }

    if hits.is_empty() {
        Ok("none".to_string())
    } else {
        Ok(hits.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn grep_finds_matches() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, "hello\nmatch me\n").unwrap();

        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
        };
        let args = json!({"pat": "match"});

        let out = tool_grep(&args, &ctx).unwrap();
        assert!(out.contains("match me"));
    }

    #[test]
    fn grep_no_matches() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, "hello\nworld\n").unwrap();

        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
        };
        let args = json!({"pat": "xyz"});

        let out = tool_grep(&args, &ctx).unwrap();
        assert_eq!(out, "none");
    }

    #[test]
    fn grep_handles_regex_patterns() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, "test123\ntest456\nhello\n").unwrap();

        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
        };
        let args = json!({"pat": "test\\d+"});

        let out = tool_grep(&args, &ctx).unwrap();
        assert!(out.contains("test123"));
        assert!(out.contains("test456"));
        assert!(!out.contains("hello"));
    }
}
