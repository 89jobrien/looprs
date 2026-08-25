use crate::{doob, git_info};
use serde::{Deserialize, Serialize};

/// Project name doob todos are queried under. Matches the `--project`
/// scoping convention used across this workspace's doob usage.
const DOOB_PROJECT: &str = "looprs";
const DOOB_TODO_LIMIT: usize = 5;

/// Context available at session start
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub git: git_info::GitInfo,
    pub doob_status: Option<doob::DoobStatus>,
}

impl SessionContext {
    /// Collect context from git and doob if available
    pub fn collect() -> Self {
        SessionContext {
            git: git_info::collect(),
            doob_status: doob::collect(DOOB_PROJECT, DOOB_TODO_LIMIT),
        }
    }

    /// Format context as a human-readable string for prompt injection
    pub fn format_for_prompt(&self) -> Option<String> {
        let mut parts = Vec::new();

        if let Some(ref branch) = self.git.branch {
            parts.push(format!(
                "## Repository Status\n- Branch: {branch}\n- Commits ahead of upstream: {}\n- Modified files: {}\n- Untracked files: {}",
                self.git.ahead, self.git.modified, self.git.untracked
            ));
        }

        if let Some(ref doob) = self.doob_status {
            let todos_str = doob
                .todos
                .iter()
                .map(|t| format!("  - (p{}) {}", t.priority, t.content))
                .collect::<Vec<_>>()
                .join("\n");
            parts.push(format!(
                "## Pending Todos ({DOOB_PROJECT})\n  Total pending: {}\n{todos_str}",
                doob.count
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
        self.git.branch.is_none() && self.doob_status.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_context_collect() {
        // Should work even with no doob available; git branch will be set
        // since this crate itself lives in a git repo.
        let ctx = SessionContext::collect();
        assert!(ctx.doob_status.is_none());
    }

    #[test]
    fn test_session_context_format_empty() {
        let ctx = SessionContext {
            git: git_info::GitInfo::default(),
            doob_status: None,
        };
        assert!(ctx.format_for_prompt().is_none());
    }

    #[test]
    fn test_session_context_format_with_git() {
        let ctx = SessionContext {
            git: git_info::GitInfo {
                branch: Some("main".to_string()),
                ahead: 2,
                modified: 1,
                untracked: 0,
            },
            doob_status: None,
        };

        let text = ctx.format_for_prompt().expect("expected formatted prompt");
        assert!(text.contains("main"));
        assert!(text.contains("Commits ahead of upstream: 2"));
    }

    #[test]
    fn test_session_context_format_with_doob() {
        let ctx = SessionContext {
            git: git_info::GitInfo::default(),
            doob_status: Some(doob::DoobStatus {
                count: 1,
                todos: vec![doob::DoobTodo {
                    content: "Fix bug".to_string(),
                    priority: 3,
                }],
            }),
        };

        let text = ctx.format_for_prompt().expect("expected formatted prompt");
        assert!(text.contains("Fix bug"));
        assert!(text.contains("Total pending: 1"));
    }

    #[test]
    fn test_session_context_is_empty() {
        let ctx = SessionContext {
            git: git_info::GitInfo::default(),
            doob_status: None,
        };
        assert!(ctx.is_empty());
    }
}
