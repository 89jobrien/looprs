use serde::{Deserialize, Serialize};
use std::ffi::OsString;

use crate::plugins::NamedTool;
use crate::plugins::binaries::Bd;

/// Represents a beads.db issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BdIssue {
    pub id: String,
    pub title: String,
    pub status: String,
    #[serde(default)]
    pub priority: Option<String>,
}

/// Get open issues from beads.db if available
pub fn list_open_issues() -> Option<Vec<BdIssue>> {
    if !is_bd_repo() {
        return None;
    }

    let output = Bd::system().output_if_available(vec![
        OsString::from("list"),
        OsString::from("--open"),
        OsString::from("--json"),
    ])?;

    if !output.status.success() {
        return None;
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    parse_bd_issues(&output_str)
}

/// Get issue count from beads.db
pub fn count_open_issues() -> Option<usize> {
    list_open_issues().map(|issues| issues.len())
}

/// Check if current directory has beads.db repo
fn is_bd_repo() -> bool {
    std::path::Path::new(".beads").exists()
}

/// Parse bd JSON output into issues
fn parse_bd_issues(json_str: &str) -> Option<Vec<BdIssue>> {
    // bd list --open --json returns newline-delimited JSON
    let mut issues = Vec::new();

    for line in json_str.lines() {
        if line.is_empty() {
            continue;
        }

        // Try to parse as JSON object
        if let Ok(issue) = serde_json::from_str::<BdIssue>(line) {
            issues.push(issue);
        }
    }

    if issues.is_empty() {
        None
    } else {
        Some(issues)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bd_issues_empty() {
        let result = parse_bd_issues("");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_bd_issues_valid() {
        let json = r#"{"id":"1","title":"test issue","status":"open"}"#;
        let result = parse_bd_issues(json);
        assert!(result.is_some());

        let issues = result.expect("expected parsed bd issues");
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, "1");
        assert_eq!(issues[0].title, "test issue");
    }

    #[test]
    fn test_parse_bd_issues_multiple() {
        let json = r#"{"id":"1","title":"issue 1","status":"open"}
{"id":"2","title":"issue 2","status":"open","priority":"high"}"#;
        let result = parse_bd_issues(json);
        assert!(result.is_some());

        let issues = result.expect("expected parsed bd issues");
        assert_eq!(issues.len(), 2);
        assert_eq!(issues[0].title, "issue 1");
        assert_eq!(issues[1].priority, Some("high".to_string()));
    }
}
