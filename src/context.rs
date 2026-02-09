use crate::{bd, jj, kan};
use serde::{Deserialize, Serialize};

/// Context available at session start
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub jj_status: Option<jj::JjStatus>,
    pub jj_recent_commits: Option<Vec<String>>,
    pub bd_open_issues: Option<Vec<bd::BdIssue>>,
    pub kan_status: Option<kan::KanStatus>,
}

impl SessionContext {
    /// Collect context from jj, bd, and kan if available
    pub fn collect() -> Self {
        SessionContext {
            jj_status: jj::get_status(),
            jj_recent_commits: jj::get_recent_commits(5),
            bd_open_issues: bd::list_open_issues(),
            kan_status: kan::get_status(),
        }
    }

    /// Format context as a human-readable string for prompt injection
    pub fn format_for_prompt(&self) -> Option<String> {
        let mut parts = Vec::new();

        // Format jj status if available
        if let Some(ref status) = self.jj_status {
            parts.push(format!(
                "## Repository Status\n- Branch: {}\n- Commit: {}\n- Description: {}",
                status.branch, status.commit, status.description
            ));
        }

        // Format recent commits if available
        if let Some(ref commits) = self.jj_recent_commits {
            let commits_str = commits
                .iter()
                .map(|c| format!("  - {c}"))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(format!("## Recent Commits\n{commits_str}"));
        }

        // Format open issues if available
        if let Some(ref issues) = self.bd_open_issues {
            let issues_str = issues
                .iter()
                .map(|i| {
                    format!(
                        "  - [{}] {}: {}",
                        i.id,
                        i.title,
                        i.priority.as_ref().unwrap_or(&"normal".to_string())
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(format!("## Open Issues\n{issues_str}"));
        }

        // Format kan status if available
        if let Some(ref kan) = self.kan_status {
            let columns_str = kan
                .by_column
                .iter()
                .map(|col| format!("  - {}: {}", col.name, col.count))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(format!(
                "## Kanban Board\n  Total tasks: {}\n{columns_str}",
                kan.total_tasks
            ));
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join("\n\n"))
        }
    }

    /// Check if there is any context available
    pub fn is_empty(&self) -> bool {
        self.jj_status.is_none()
            && self.jj_recent_commits.is_none()
            && self.bd_open_issues.is_none()
            && self.kan_status.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_context_collect() {
        // Should work even with no jj/bd repo
        let ctx = SessionContext::collect();
        assert!(ctx.is_empty()); // No jj/bd in current env
    }

    #[test]
    fn test_session_context_format_empty() {
        let ctx = SessionContext {
            jj_status: None,
            jj_recent_commits: None,
            bd_open_issues: None,
            kan_status: None,
        };
        assert!(ctx.format_for_prompt().is_none());
    }

    #[test]
    fn test_session_context_format_with_jj() {
        let ctx = SessionContext {
            jj_status: Some(jj::JjStatus {
                branch: "main".to_string(),
                commit: "abc123".to_string(),
                description: "Initial commit".to_string(),
            }),
            jj_recent_commits: None,
            bd_open_issues: None,
            kan_status: None,
        };

        let formatted = ctx.format_for_prompt();
        assert!(formatted.is_some());
        let text = formatted.unwrap();
        assert!(text.contains("main"));
        assert!(text.contains("abc123"));
    }

    #[test]
    fn test_session_context_format_with_issues() {
        let ctx = SessionContext {
            jj_status: None,
            jj_recent_commits: None,
            bd_open_issues: Some(vec![bd::BdIssue {
                id: "1".to_string(),
                title: "Test issue".to_string(),
                status: "open".to_string(),
                priority: Some("high".to_string()),
            }]),
            kan_status: None,
        };

        let formatted = ctx.format_for_prompt();
        assert!(formatted.is_some());
        let text = formatted.unwrap();
        assert!(text.contains("Test issue"));
        assert!(text.contains("high"));
    }
}
