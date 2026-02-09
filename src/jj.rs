use serde::{Deserialize, Serialize};
use std::process::Command;

/// Represents jujutsu repository status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JjStatus {
    pub branch: String,
    pub commit: String,
    pub description: String,
}

/// Get jujutsu repository status if available
pub fn get_status() -> Option<JjStatus> {
    if !is_jj_repo() {
        return None;
    }

    let branch = get_branch()?;
    let commit = get_current_commit()?;
    let description = get_current_description()?;

    Some(JjStatus {
        branch,
        commit,
        description,
    })
}

/// Get recent commits from jujutsu repo
pub fn get_recent_commits(count: usize) -> Option<Vec<String>> {
    if !is_jj_repo() {
        return None;
    }

    let output = Command::new("jj")
        .args([
            "log",
            "--no-pager",
            "-r",
            "main..",
            "-n",
            &count.to_string(),
            "-T",
            r#"description.first_line()"#,
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let commits: Vec<String> = output_str
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.trim().to_string())
        .collect();

    if commits.is_empty() {
        None
    } else {
        Some(commits)
    }
}

/// Check if current directory is a jujutsu repository
fn is_jj_repo() -> bool {
    std::path::Path::new(".jj").exists()
}

/// Get current branch name from jj
fn get_branch() -> Option<String> {
    let output = Command::new("jj")
        .args(["branch", "list", "-q"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if branch.is_empty() {
        None
    } else {
        Some(branch)
    }
}

/// Get current commit ID from jj
fn get_current_commit() -> Option<String> {
    let output = Command::new("jj")
        .args([
            "log",
            "-r",
            "@",
            "--no-pager",
            "-T",
            r#"{change_id.short()}"#,
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let commit = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if commit.is_empty() {
        None
    } else {
        Some(commit)
    }
}

/// Get current commit description from jj
fn get_current_description() -> Option<String> {
    let output = Command::new("jj")
        .args(["log", "-r", "@", "--no-pager", "-T", r#"{description}"#])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let description = String::from_utf8_lossy(&output.stdout).trim().to_string();

    if description.is_empty() {
        None
    } else {
        Some(description)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jj_repo_detection() {
        // This will be false in current git repo
        assert!(!is_jj_repo());
    }

    #[test]
    fn test_get_status_no_jj_repo() {
        // Should return None when not in jj repo
        let status = get_status();
        assert!(status.is_none());
    }

    #[test]
    fn test_get_recent_commits_no_jj_repo() {
        // Should return None when not in jj repo
        let commits = get_recent_commits(5);
        assert!(commits.is_none());
    }
}
