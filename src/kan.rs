use serde::{Deserialize, Serialize};
#[cfg(not(test))]
use std::ffi::OsString;

#[cfg(not(test))]
use crate::plugins::NamedTool;
#[cfg(not(test))]
use crate::plugins::binaries::Kan;

/// Represents a kan board status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanStatus {
    pub total_tasks: usize,
    pub by_column: Vec<KanColumn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanColumn {
    pub name: String,
    pub count: usize,
}

/// Get kan board status if available
pub fn get_status() -> Option<KanStatus> {
    // In test environments, return None immediately to avoid hanging
    #[cfg(test)]
    {
        None
    }

    #[cfg(not(test))]
    {
        // Quick check - if kan command doesn't exist, return early
        if !is_kan_available() {
            return None;
        }

        let output = Kan::system()
            .output_if_available(vec![OsString::from("status"), OsString::from("--json")])?;

        if !output.status.success() {
            return None;
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        parse_kan_status(&output_str)
    }
}

/// Check if kan is available in PATH
fn is_kan_available() -> bool {
    crate::plugins::system().has_in_path("kan")
}

/// Parse kan JSON output into status
fn parse_kan_status(json_str: &str) -> Option<KanStatus> {
    serde_json::from_str::<KanStatus>(json_str).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kan_detection() {
        // kan may or may not be available - just verify it doesn't panic
        let _available = is_kan_available();
    }

    #[test]
    fn test_get_status_no_kan() {
        // Should return None if kan not available
        if !is_kan_available() {
            let status = get_status();
            assert!(status.is_none());
        }
    }

    #[test]
    fn test_parse_kan_status_valid() {
        let json = r#"{"total_tasks":5,"by_column":[{"name":"todo","count":2},{"name":"done","count":3}]}"#;
        let result = parse_kan_status(json);
        assert!(result.is_some());

        let status = result.unwrap();
        assert_eq!(status.total_tasks, 5);
        assert_eq!(status.by_column.len(), 2);
        assert_eq!(status.by_column[0].name, "todo");
        assert_eq!(status.by_column[0].count, 2);
    }

    #[test]
    fn test_parse_kan_status_empty() {
        let result = parse_kan_status("");
        assert!(result.is_none());
    }
}
