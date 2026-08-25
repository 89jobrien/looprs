//! Pending-doob-todo collection for session-start context.
//!
//! Thin adapter over the external `doob` CLI (`doob todo list --json`);
//! returns `None` whenever `doob` is missing or fails, so context
//! collection degrades gracefully.

use serde::{Deserialize, Serialize};
#[cfg(not(test))]
use std::ffi::OsString;

#[cfg(not(test))]
use crate::plugins::NamedTool;
#[cfg(not(test))]
use crate::plugins::binaries::Doob;

/// Summary of pending doob todos for a project, for session-start context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoobStatus {
    /// Number of pending todos; matches `todos.len()` when parsing
    /// succeeds, but is taken from the CLI output rather than recomputed.
    pub count: usize,
    /// Pending todos, ordered as returned by the `doob` CLI.
    pub todos: Vec<DoobTodo>,
}

/// A single pending doob todo, reduced to what prompt injection needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoobTodo {
    /// Todo title/body text.
    pub content: String,
    /// Doob priority (1 highest to 5 lowest).
    pub priority: i64,
}

/// Collect pending doob todos scoped to `project`, if `doob` is available.
///
/// Returns `None` when the binary is missing, the command fails, or its
/// output cannot be parsed — context collection treats all of these as
/// "no doob data" rather than an error.
pub fn collect(project: &str, limit: usize) -> Option<DoobStatus> {
    // In test environments, return None immediately to avoid hanging.
    #[cfg(test)]
    {
        let _ = (project, limit);
        None
    }

    #[cfg(not(test))]
    {
        if !is_doob_available() {
            return None;
        }

        let output = Doob::system().output_if_available(vec![
            OsString::from("todo"),
            OsString::from("list"),
            OsString::from("--json"),
            OsString::from("--status"),
            OsString::from("pending"),
            OsString::from("--project"),
            OsString::from(project),
            OsString::from("--limit"),
            OsString::from(limit.to_string()),
        ])?;

        if !output.status.success() {
            return None;
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        parse_doob_list(&output_str)
    }
}

/// Check if doob is available in PATH.
#[cfg(not(test))]
fn is_doob_available() -> bool {
    crate::plugins::system().has_in_path("doob")
}

#[derive(Deserialize)]
struct DoobListResponse {
    count: usize,
    todos: Vec<DoobListTodo>,
}

#[derive(Deserialize)]
struct DoobListTodo {
    content: String,
    priority: i64,
}

/// Parse `doob todo list --json` output into a status summary.
fn parse_doob_list(json_str: &str) -> Option<DoobStatus> {
    let parsed: DoobListResponse = serde_json::from_str(json_str).ok()?;
    Some(DoobStatus {
        count: parsed.count,
        todos: parsed
            .todos
            .into_iter()
            .map(|t| DoobTodo {
                content: t.content,
                priority: t.priority,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_none_in_tests() {
        let status = collect("looprs", 5);
        assert!(status.is_none());
    }

    #[test]
    fn test_parse_doob_list_valid() {
        let json = r#"{"count":2,"todos":[{"id":{"tb":"todo","id":{"String":"abc"}},"uuid":"u1","content":"Fix bug","status":"pending","priority":3,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","completed_at":null,"due_date":null,"project":"looprs","project_path":null,"file_path":null,"tags":[],"metadata":null,"blocks":[],"blocked_by":[]},{"id":{"tb":"todo","id":{"String":"def"}},"uuid":"u2","content":"Write docs","status":"pending","priority":1,"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","completed_at":null,"due_date":null,"project":"looprs","project_path":null,"file_path":null,"tags":[],"metadata":null,"blocks":[],"blocked_by":[]}]}"#;
        let result = parse_doob_list(json);
        assert!(result.is_some());

        let status = result.expect("expected parsed doob status");
        assert_eq!(status.count, 2);
        assert_eq!(status.todos.len(), 2);
        assert_eq!(status.todos[0].content, "Fix bug");
        assert_eq!(status.todos[0].priority, 3);
    }

    #[test]
    fn test_parse_doob_list_empty() {
        let result = parse_doob_list("");
        assert!(result.is_none());
    }
}
