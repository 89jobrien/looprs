use serde::{Deserialize, Serialize};
use std::process::Command;

/// Git repository state for session-start context.
///
/// All counts are best-effort: any failed `git` invocation degrades to a
/// default value rather than an error.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitInfo {
    pub branch: Option<String>,
    pub ahead: u32,
    pub modified: u32,
    pub untracked: u32,
}

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
        Ok(o) if o.status.success() => parse_porcelain(&String::from_utf8_lossy(&o.stdout)),
        _ => (0, 0),
    }
}

/// Count modified vs untracked entries from `git status --porcelain` output.
///
/// Untracked lines start with `??`; every other non-empty line counts as
/// modified (including renames/copies and staged changes).
fn parse_porcelain(stdout: &str) -> (u32, u32) {
    let mut modified = 0u32;
    let mut untracked = 0u32;
    for line in stdout.lines() {
        if line.starts_with("??") {
            untracked += 1;
        } else if !line.is_empty() {
            modified += 1;
        }
    }
    (modified, untracked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_empty_output_counts_nothing() {
        assert_eq!(parse_porcelain(""), (0, 0));
    }

    #[test]
    fn porcelain_untracked_lines_are_counted_separately() {
        let out = "?? new_file.rs\n?? other.txt\n";
        assert_eq!(parse_porcelain(out), (0, 2));
    }

    #[test]
    fn porcelain_mixed_statuses() {
        let out = " M src/lib.rs\n?? notes.md\nM  Cargo.toml\n";
        assert_eq!(parse_porcelain(out), (2, 1));
    }

    #[test]
    fn porcelain_rename_line_counts_as_modified() {
        let out = "R  old.rs -> new.rs\n";
        assert_eq!(parse_porcelain(out), (1, 0));
    }

    #[test]
    fn porcelain_trailing_blank_line_is_ignored() {
        let out = " M a.rs\n\n";
        assert_eq!(parse_porcelain(out), (1, 0));
    }

    /// Smoke test: collection degrades gracefully. Inside the repo it should
    /// report real values; outside any repo every field falls back to defaults.
    #[test]
    fn collect_returns_without_panicking() {
        let info = collect();
        if let Some(branch) = &info.branch {
            assert!(!branch.is_empty());
        }
    }
}
