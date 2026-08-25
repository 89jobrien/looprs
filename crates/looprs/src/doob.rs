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

#[cfg(not(test))]
use std::sync::mpsc;
#[cfg(not(test))]
use std::thread;
#[cfg(not(test))]
use std::time::Duration;

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

const MAX_TODO_CONTENT: usize = 500;
#[cfg(not(test))]
const DOOB_TIMEOUT_SECS: u64 = 2;

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
            // benign: doob missing means no contextual todos
            eprintln!("doob: not found in PATH; skipping doob context collection");
            return None;
        }

        // Run the plugin call on a background thread and wait with a timeout.
        let (tx, rx) = mpsc::channel();
        let project = project.to_string();

        let _handle = thread::spawn(move || {
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
            ]);
            let _ = tx.send(output);
        });

        match rx.recv_timeout(Duration::from_secs(DOOB_TIMEOUT_SECS)) {
            Ok(Some(output)) => {
                if !output.status.success() {
                    eprintln!("doob: command returned non-success status; skipping");
                    return None;
                }
                let output_str = String::from_utf8_lossy(&output.stdout);
                parse_doob_list(&output_str)
            }
            Ok(None) => {
                // Doob wrapper indicated the binary wasn't available or couldn't run
                eprintln!("doob: output not available from Doob::system(); skipping");
                None
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                eprintln!(
                    "doob: command timed out after {}s; skipping",
                    DOOB_TIMEOUT_SECS
                );
                None
            }
            Err(e) => {
                eprintln!("doob: channel recv error: {e}; skipping");
                None
            }
        }
    }
}

/// Check if doob is available in PATH.
#[cfg(not(test))]
fn is_doob_available() -> bool {
    crate::plugins::system().has_in_path("doob")
}

#[derive(Deserialize)]
struct DoobListResponse {
    count: Option<usize>,
    todos: Option<Vec<DoobListTodo>>,
}

#[derive(Deserialize)]
struct DoobListTodo {
    content: Option<String>,
    priority: Option<i64>,
}

/// Parse `doob todo list --json` output into a status summary.
///
/// This parser is forgiving: missing fields are handled with sensible
/// defaults. Additionally, todo content is sanitized and truncated to avoid
/// unbounded prompt bloat or sneaking control characters into prompts.
fn parse_doob_list(json_str: &str) -> Option<DoobStatus> {
    let parsed: DoobListResponse = serde_json::from_str(json_str).ok()?;
    let todos = parsed.todos.unwrap_or_default();

    let mut out_todos = Vec::new();
    for t in todos.into_iter() {
        let raw = t.content.unwrap_or_default();
        let sanitized: String = raw
            .chars()
            .filter(|c| {
                // allow printable chars plus common whitespace (LF, CR, TAB)
                !c.is_control() || *c == '\n' || *c == '\r' || *c == '\t'
            })
            .collect();
        let content = if sanitized.chars().count() > MAX_TODO_CONTENT {
            let mut s = sanitized
                .chars()
                .take(MAX_TODO_CONTENT - 1)
                .collect::<String>();
            s.push('\u{2026}'); // ellipsis
            s
        } else {
            sanitized
        };

        let priority = t.priority.unwrap_or(5);
        out_todos.push(DoobTodo { content, priority });
    }

    let count = parsed.count.unwrap_or(out_todos.len());

    Some(DoobStatus {
        count,
        todos: out_todos,
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

    #[test]
    fn test_parse_doob_list_missing_fields() {
        let json = r#"{"todos":[{"content":null,"priority":null},{"content":"x"}]}"#;
        let result = parse_doob_list(json).expect("parse should succeed");
        assert_eq!(result.count, 2);
        assert_eq!(result.todos.len(), 2);
        assert_eq!(result.todos[0].content, "");
        assert_eq!(result.todos[0].priority, 5);
        assert_eq!(result.todos[1].content, "x");
    }

    #[test]
    fn test_content_truncation_and_sanitize() {
        let long = "a".repeat(MAX_TODO_CONTENT + 50);
        let bad = format!(r#"{{"todos":[{{"content":"{}"}}]}}"#, long);
        let result = parse_doob_list(&bad).expect("parse should succeed");
        assert_eq!(result.todos.len(), 1);
        assert!(result.todos[0].content.chars().count() <= MAX_TODO_CONTENT);
        // ensure ends with ellipsis char when truncated
        if result.todos[0].content.chars().count() == MAX_TODO_CONTENT {
            assert!(
                result.todos[0]
                    .content
                    .chars()
                    .last()
                    .map(|c| c == '\u{2026}')
                    .unwrap_or(false)
            );
        }
    }
}
