//! Collects best-effort Git status information for the UI statusline.

use std::process::Command;

/// A snapshot of the current repo's git state, for display in the
/// statusline (see `crate::ui::statusline_prompt_statusline`).
#[derive(Debug, Default, Clone)]
pub struct GitInfo {
    /// Current branch name, or `None` if detached HEAD, not a git repo, or
    /// the `git` command failed.
    pub branch: Option<String>,
    /// Number of commits the current branch is ahead of its upstream, or
    /// `0` if there is no upstream or the count could not be determined.
    pub ahead: u32,
    /// Number of tracked files with uncommitted changes.
    pub modified: u32,
    /// Number of untracked files.
    pub untracked: u32,
}

/// Gathers a [`GitInfo`] snapshot by shelling out to `git`. Best-effort:
/// any individual `git` invocation that fails (e.g. not a git repository)
/// yields the corresponding field's default value rather than an error.
pub fn collect() -> GitInfo {
    let branch = branch_name();
    let ahead = commits_ahead();
    let (modified, untracked) = changed_files();
    GitInfo {
        branch,
        ahead,
        modified,
        untracked,
    }
}

fn branch_name() -> Option<String> {
    let out = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() || s == "HEAD" {
            None
        } else {
            Some(s)
        }
    } else {
        None
    }
}

fn commits_ahead() -> u32 {
    let out = Command::new("git")
        .args(["rev-list", "--count", "@{u}..HEAD"])
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .trim()
            .parse()
            .unwrap_or(0),
        _ => 0,
    }
}

fn changed_files() -> (u32, u32) {
    let out = Command::new("git").args(["status", "--porcelain"]).output();
    match out {
        Ok(o) if o.status.success() => {
            let mut modified = 0u32;
            let mut untracked = 0u32;
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                if line.starts_with("??") {
                    untracked += 1;
                } else if !line.is_empty() {
                    modified += 1;
                }
            }
            (modified, untracked)
        }
        _ => (0, 0),
    }
}
