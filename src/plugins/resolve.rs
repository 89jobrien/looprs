use std::path::{Path, PathBuf};

use super::registry::ToolResolver;

pub struct PathResolver;

impl ToolResolver for PathResolver {
    fn resolve(&self, tool: &str) -> Option<PathBuf> {
        find_in_path(tool)
    }
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path)
            .map(|m| (m.permissions().mode() & 0o111) != 0)
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    {
        // Best-effort: on non-unix, existence is our proxy.
        true
    }
}

/// Resolve a tool by searching PATH.
pub fn find_in_path(tool: &str) -> Option<PathBuf> {
    if tool.is_empty() {
        return None;
    }

    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(tool);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    #[test]
    fn find_in_path_handles_missing() {
        // Unlikely to exist.
        assert!(find_in_path("totally_nonexistent_tool_xyz").is_none());
    }

    #[test]
    fn find_in_path_finds_executable_in_temp_path() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let _lock = ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
            let old_path = std::env::var_os("PATH");

            let dir = tempfile::tempdir().unwrap();
            let tool_path = dir.path().join("mytool");
            std::fs::write(&tool_path, "#!/bin/sh\necho ok\n").unwrap();
            let mut perms = std::fs::metadata(&tool_path).unwrap().permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&tool_path, perms).unwrap();

            // SAFETY: env mutation is guarded by ENV_LOCK.
            unsafe {
                std::env::set_var("PATH", dir.path());
            }

            let resolved = find_in_path("mytool");
            assert_eq!(resolved, Some(tool_path));

            // Restore PATH
            // SAFETY: env mutation is guarded by ENV_LOCK.
            unsafe {
                match old_path {
                    Some(v) => std::env::set_var("PATH", v),
                    None => std::env::remove_var("PATH"),
                }
            }
        }
    }
}
