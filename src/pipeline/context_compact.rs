use crate::app_config::PipelineCompactionConfig;
use anyhow::Result;
use glob::{glob, Pattern};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

const MAX_FILE_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Default)]
pub struct CompactedContext {
    pub text: String,
    pub files: Vec<String>,
}

pub fn compact_context(
    repo_root: &Path,
    config: &PipelineCompactionConfig,
) -> Result<CompactedContext> {
    let mut ordered = Vec::new();
    let mut seen = HashSet::new();

    if config.include_diff {
        add_paths(
            &mut ordered,
            &mut seen,
            repo_root,
            git_diff_files(repo_root),
        );
    }
    if config.include_recent {
        add_paths(
            &mut ordered,
            &mut seen,
            repo_root,
            git_status_files(repo_root),
        );
    }
    if !config.include_globs.is_empty() {
        add_paths(
            &mut ordered,
            &mut seen,
            repo_root,
            glob_files(repo_root, &config.include_globs),
        );
    }
    if config.top_k > 0 {
        add_paths(
            &mut ordered,
            &mut seen,
            repo_root,
            top_k_files(repo_root, &config.include_globs, config.top_k),
        );
    }

    let mut text = String::new();
    let mut files = Vec::new();
    for path in ordered {
        let rel = path.strip_prefix(repo_root).unwrap_or(&path);
        let display = rel.to_string_lossy();
        files.push(display.to_string());

        text.push_str(&format!("// File: {display}\n"));
        match fs::metadata(&path) {
            Ok(meta) if meta.len() > MAX_FILE_BYTES => {
                text.push_str("// Skipped: file too large\n\n");
                continue;
            }
            Ok(_) => {}
            Err(_) => {
                text.push_str("// Skipped: unreadable\n\n");
                continue;
            }
        }

        let content = fs::read(&path)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok());
        if let Some(content) = content {
            text.push_str(&content);
            if !content.ends_with('\n') {
                text.push('\n');
            }
        } else {
            text.push_str("// Skipped: non-utf8 or unreadable\n");
        }
        text.push('\n');
    }

    Ok(CompactedContext { text, files })
}

fn add_paths(
    ordered: &mut Vec<PathBuf>,
    seen: &mut HashSet<PathBuf>,
    repo_root: &Path,
    paths: Vec<PathBuf>,
) {
    for path in paths {
        let abs = if path.is_absolute() {
            path
        } else {
            repo_root.join(path)
        };
        if !abs.is_file() {
            continue;
        }
        if seen.insert(abs.clone()) {
            ordered.push(abs);
        }
    }
}

fn git_diff_files(repo_root: &Path) -> Vec<PathBuf> {
    let output = Command::new("git")
        .args(["diff", "--name-only"])
        .current_dir(repo_root)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    parse_lines(&output.stdout)
}

fn git_status_files(repo_root: &Path) -> Vec<PathBuf> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repo_root)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    let mut paths = Vec::new();
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if line.len() < 3 {
            continue;
        }
        let mut path = line[3..].trim();
        if let Some((_, new)) = path.rsplit_once("->") {
            path = new.trim();
        }
        if !path.is_empty() {
            paths.push(PathBuf::from(path));
        }
    }
    paths
}

fn parse_lines(buf: &[u8]) -> Vec<PathBuf> {
    let stdout = String::from_utf8_lossy(buf);
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .collect()
}

fn glob_files(repo_root: &Path, globs: &[String]) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let repo_root_canon = fs::canonicalize(repo_root).unwrap_or_else(|_| repo_root.to_path_buf());
    for glob_str in globs {
        let pattern = repo_root.join(glob_str);
        let pattern = pattern.to_string_lossy().to_string();
        if let Ok(entries) = glob(&pattern) {
            for entry in entries.flatten() {
                if entry.is_file() {
                    let entry_canon = fs::canonicalize(&entry).unwrap_or(entry.clone());
                    if entry_canon.starts_with(&repo_root_canon) {
                        paths.push(entry);
                    }
                }
            }
        }
    }
    sort_unique_paths(paths)
}

fn top_k_files(repo_root: &Path, globs: &[String], top_k: usize) -> Vec<PathBuf> {
    let candidates =
        rg_files(repo_root, globs).unwrap_or_else(|| list_files_fallback(repo_root, globs));
    let mut with_time: Vec<(u128, PathBuf)> = candidates
        .into_iter()
        .map(|path| {
            let mtime = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            (mtime, path)
        })
        .collect();
    with_time.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    with_time.truncate(top_k);
    with_time.into_iter().map(|(_, path)| path).collect()
}

fn rg_files(repo_root: &Path, globs: &[String]) -> Option<Vec<PathBuf>> {
    let mut cmd = Command::new("rg");
    cmd.arg("--files");
    for glob_str in globs {
        cmd.args(["-g", glob_str]);
    }
    let output = cmd.current_dir(repo_root).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let mut paths = parse_lines(&output.stdout)
        .into_iter()
        .map(|path| repo_root.join(path))
        .collect::<Vec<_>>();
    paths.retain(|path| path.is_file());
    Some(paths)
}

fn list_files_fallback(repo_root: &Path, globs: &[String]) -> Vec<PathBuf> {
    let patterns: Vec<Pattern> = globs.iter().filter_map(|g| Pattern::new(g).ok()).collect();
    let mut paths = Vec::new();
    let walker = WalkDir::new(repo_root)
        .into_iter()
        .filter_entry(|entry| entry.file_name().to_str() != Some(".git"));
    for entry in walker.flatten() {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let rel = path.strip_prefix(repo_root).unwrap_or(path);
        let matches = patterns.is_empty() || patterns.iter().any(|p| p.matches_path(rel));
        if matches {
            paths.push(path.to_path_buf());
        }
    }
    sort_unique_paths(paths)
}

fn sort_unique_paths(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort_by(|a, b| a.to_string_lossy().cmp(&b.to_string_lossy()));
    paths.dedup();
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn run_git(repo_root: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(repo_root)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .status()
            .unwrap();
        assert!(status.success(), "git {:?} failed", args);
    }

    fn init_git_repo(repo_root: &Path) {
        run_git(repo_root, &["init"]);
        run_git(repo_root, &["config", "user.email", "test@example.com"]);
        run_git(repo_root, &["config", "user.name", "Test User"]);
    }

    #[test]
    fn test_compact_context_includes_diff_and_globs() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::write(repo.path().join("a.txt"), "hello").unwrap();
        let config = PipelineCompactionConfig {
            include_diff: true,
            include_recent: true,
            include_globs: vec!["*.txt".to_string()],
            top_k: 0,
        };
        let out = compact_context(repo.path(), &config).unwrap();
        assert!(out.text.contains("a.txt"));
    }

    #[test]
    fn test_compact_context_rejects_glob_escape() {
        let repo = tempfile::tempdir().unwrap();
        let inside_path = repo.path().join("inside.txt");
        std::fs::write(&inside_path, "inside").unwrap();

        let outside = tempfile::tempdir().unwrap();
        let outside_path = outside.path().join("outside.txt");
        std::fs::write(&outside_path, "outside").unwrap();

        let config = PipelineCompactionConfig {
            include_diff: false,
            include_recent: false,
            include_globs: vec![outside_path.to_string_lossy().to_string()],
            top_k: 0,
        };
        let out = compact_context(repo.path(), &config).unwrap();
        assert!(!out.text.contains("outside"));
        assert!(!out.text.contains(outside_path.to_string_lossy().as_ref()));
    }

    #[test]
    fn test_compact_context_includes_git_diff_files() {
        let repo = tempfile::tempdir().unwrap();
        init_git_repo(repo.path());

        let file_path = repo.path().join("tracked.txt");
        std::fs::write(&file_path, "initial").unwrap();
        run_git(repo.path(), &["add", "tracked.txt"]);
        run_git(repo.path(), &["commit", "-m", "init"]);

        std::fs::write(&file_path, "changed").unwrap();

        let config = PipelineCompactionConfig {
            include_diff: true,
            include_recent: false,
            include_globs: Vec::new(),
            top_k: 0,
        };
        let out = compact_context(repo.path(), &config).unwrap();
        assert!(out.files.iter().any(|file| file == "tracked.txt"));
        assert!(out.text.contains("tracked.txt"));
    }

    #[test]
    fn test_compact_context_includes_recent_status_files() {
        let repo = tempfile::tempdir().unwrap();
        init_git_repo(repo.path());

        let file_path = repo.path().join("tracked.txt");
        std::fs::write(&file_path, "initial").unwrap();
        run_git(repo.path(), &["add", "tracked.txt"]);
        run_git(repo.path(), &["commit", "-m", "init"]);

        std::fs::write(&file_path, "changed").unwrap();

        let config = PipelineCompactionConfig {
            include_diff: false,
            include_recent: true,
            include_globs: Vec::new(),
            top_k: 0,
        };
        let out = compact_context(repo.path(), &config).unwrap();
        assert!(out.files.iter().any(|file| file == "tracked.txt"));
        assert!(out.text.contains("tracked.txt"));
    }
}
