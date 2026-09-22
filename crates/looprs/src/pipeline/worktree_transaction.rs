//! Captures and rolls back Git worktree state around tool execution.

use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

#[derive(Debug)]
struct UntrackedFile {
    path: PathBuf,
    contents: Vec<u8>,
}

/// Snapshot of Git-managed worktree state captured before a tool round-trip.
///
/// Rollback restores the index, tracked worktree files, and non-ignored
/// untracked files to this baseline. Ignored files and nested repositories are
/// intentionally outside the transaction boundary.
#[derive(Debug)]
pub(crate) struct WorktreeTransaction {
    root: PathBuf,
    staged_patch: Vec<u8>,
    unstaged_patch: Vec<u8>,
    untracked: Vec<UntrackedFile>,
}

impl WorktreeTransaction {
    pub(crate) fn capture(working_dir: &Path) -> Result<Self> {
        let root_output = git_output(working_dir, &["rev-parse", "--show-toplevel"])
            .context("capture rollback baseline: locate Git worktree")?;
        let root = PathBuf::from(
            String::from_utf8(root_output)
                .context("capture rollback baseline: worktree path is not UTF-8")?
                .trim(),
        );
        let staged_patch = git_output(
            &root,
            &[
                "diff",
                "--cached",
                "--binary",
                "--full-index",
                "--no-ext-diff",
            ],
        )
        .context("capture rollback baseline: read staged changes")?;
        let unstaged_patch = git_output(
            &root,
            &["diff", "--binary", "--full-index", "--no-ext-diff"],
        )
        .context("capture rollback baseline: read unstaged changes")?;
        let paths = untracked_paths(&root).context("capture rollback baseline: list untracked")?;
        let mut untracked = Vec::with_capacity(paths.len());
        for path in paths {
            let contents = fs::read(root.join(&path))
                .with_context(|| format!("capture rollback baseline: read {}", path.display()))?;
            untracked.push(UntrackedFile { path, contents });
        }

        Ok(Self {
            root,
            staged_patch,
            unstaged_patch,
            untracked,
        })
    }

    pub(crate) fn rollback(self) -> Result<()> {
        self.rollback_inner()
            .context("worktree rollback failed; turn changes may remain on disk")
    }

    fn rollback_inner(&self) -> Result<()> {
        let baseline_untracked = self
            .untracked
            .iter()
            .map(|file| file.path.as_path())
            .collect::<HashSet<_>>();
        let current_untracked = untracked_paths(&self.root)?;

        git_status(&self.root, &["reset", "--hard", "--quiet", "HEAD"])?;
        for path in current_untracked {
            if !baseline_untracked.contains(path.as_path()) {
                remove_untracked_path(&self.root, &path)?;
            }
        }
        apply_patch(&self.root, &self.staged_patch, true)?;
        apply_patch(&self.root, &self.unstaged_patch, false)?;
        for file in &self.untracked {
            let path = self.root.join(&file.path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(path, &file.contents)?;
        }
        Ok(())
    }
}

fn git_output(working_dir: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(working_dir)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(output.stdout)
}

fn git_status(working_dir: &Path, args: &[&str]) -> Result<()> {
    git_output(working_dir, args).map(|_| ())
}

fn untracked_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let output = git_output(root, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    output
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            let path = std::str::from_utf8(path).context("untracked path is not UTF-8")?;
            Ok(PathBuf::from(path))
        })
        .collect()
}

fn apply_patch(root: &Path, patch: &[u8], update_index: bool) -> Result<()> {
    if patch.is_empty() {
        return Ok(());
    }
    let mut command = Command::new("git");
    command.arg("apply").arg("--binary");
    if update_index {
        command.arg("--index");
    }
    let mut child = command
        .current_dir(root)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("start git apply")?;
    child
        .stdin
        .take()
        .context("open git apply stdin")?
        .write_all(patch)
        .context("write git patch")?;
    let output = child.wait_with_output().context("wait for git apply")?;
    if !output.status.success() {
        bail!(
            "git apply failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn remove_untracked_path(root: &Path, relative: &Path) -> Result<()> {
    let path = root.join(relative);
    if path.is_dir() && !path.is_symlink() {
        fs::remove_dir_all(&path)?;
    } else if path.exists() || path.is_symlink() {
        fs::remove_file(&path)?;
    }
    let mut parent = path.parent();
    while let Some(directory) = parent.filter(|directory| *directory != root) {
        if fs::remove_dir(directory).is_err() {
            break;
        }
        parent = directory.parent();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    use super::WorktreeTransaction;

    fn git(repo: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(repo)
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?} failed");
    }

    fn repo() -> tempfile::TempDir {
        let repo = tempfile::tempdir().expect("temp repo");
        git(repo.path(), &["init", "--quiet"]);
        git(repo.path(), &["config", "user.email", "test@example.com"]);
        git(repo.path(), &["config", "user.name", "Test"]);
        fs::write(repo.path().join("tracked.txt"), "original\n").expect("write tracked");
        git(repo.path(), &["add", "tracked.txt"]);
        git(repo.path(), &["commit", "--quiet", "-m", "initial"]);
        repo
    }

    fn status(repo: &Path) -> Vec<u8> {
        Command::new("git")
            .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
            .current_dir(repo)
            .output()
            .expect("git status")
            .stdout
    }

    #[test]
    fn rollback_restores_clean_worktree_and_removes_turn_files() {
        let repo = repo();
        let transaction = WorktreeTransaction::capture(repo.path()).expect("capture");

        fs::write(repo.path().join("tracked.txt"), "turn edit\n").expect("edit tracked");
        fs::write(repo.path().join("created.txt"), "turn file\n").expect("create file");
        transaction.rollback().expect("rollback");

        assert_eq!(
            fs::read_to_string(repo.path().join("tracked.txt")).unwrap(),
            "original\n"
        );
        assert!(!repo.path().join("created.txt").exists());
        assert!(status(repo.path()).is_empty());
    }

    #[test]
    fn rollback_preserves_dirty_untracked_and_partially_staged_baseline() {
        let repo = repo();
        fs::write(repo.path().join("tracked.txt"), "staged\n").expect("stage edit");
        git(repo.path(), &["add", "tracked.txt"]);
        fs::write(repo.path().join("tracked.txt"), "unstaged\n").expect("unstaged edit");
        fs::write(repo.path().join("existing.txt"), "user data\n").expect("untracked file");
        let baseline = status(repo.path());
        let transaction = WorktreeTransaction::capture(repo.path()).expect("capture");

        fs::write(repo.path().join("tracked.txt"), "turn edit\n").expect("turn edit");
        fs::write(repo.path().join("existing.txt"), "turn overwrite\n").expect("overwrite");
        fs::write(repo.path().join("created.txt"), "turn file\n").expect("create file");
        transaction.rollback().expect("rollback");

        assert_eq!(status(repo.path()), baseline);
        assert_eq!(
            fs::read_to_string(repo.path().join("tracked.txt")).unwrap(),
            "unstaged\n"
        );
        assert_eq!(
            fs::read_to_string(repo.path().join("existing.txt")).unwrap(),
            "user data\n"
        );
        let staged = Command::new("git")
            .args(["show", ":tracked.txt"])
            .current_dir(repo.path())
            .output()
            .expect("read index");
        assert_eq!(staged.stdout, b"staged\n");
    }

    #[test]
    fn rollback_reports_failure_without_claiming_success() {
        let repo = repo();
        let transaction = WorktreeTransaction::capture(repo.path()).expect("capture");
        fs::rename(repo.path().join(".git"), repo.path().join("git-disabled"))
            .expect("disable repository");

        let error = transaction.rollback().expect_err("rollback must fail");

        assert!(error.to_string().contains("rollback"));
    }
}
