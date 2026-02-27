use super::error::ToolError;
use super::ToolArgs;
use super::ToolContext;
use serde_json::Value;
use std::fs;

pub(super) fn tool_glob(args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
    let args = ToolArgs::new(args);
    let pattern = args.get_str("pat")?;
    let path_prefix = args.get_str_optional("path")?.unwrap_or(".");

    // Prevent escaping the base directory via the pattern itself.
    let pat_path = std::path::Path::new(pattern);
    if pat_path.is_absolute()
        || pat_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(ToolError::PathOutsideWorkingDir(pattern.to_string()));
    }

    let base = ctx.resolve_path(path_prefix)?;
    let full_pattern = base.join(pattern);
    let pattern_str = full_pattern
        .to_str()
        .ok_or_else(|| ToolError::InvalidPath(pattern.to_string()))?;

    let mut paths: Vec<_> = glob::glob(pattern_str)?.filter_map(Result::ok).collect();

    paths.sort_by(|a, b| {
        let m_a = fs::metadata(a).and_then(|m| m.modified()).ok();
        let m_b = fs::metadata(b).and_then(|m| m.modified()).ok();
        m_b.cmp(&m_a)
    });

    if paths.is_empty() {
        Ok("none".to_string())
    } else {
        Ok(paths
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn glob_finds_files() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.txt");
        let b = dir.path().join("b.txt");
        fs::write(&a, "a").unwrap();
        fs::write(&b, "b").unwrap();

        let ctx =
            ToolContext::from_working_dir(dir.path().to_path_buf(), crate::fs_mode::FsMode::Write);
        let args = json!({"pat": "*.txt"});

        let out = tool_glob(&args, &ctx).unwrap();
        assert!(out.contains("a.txt"));
        assert!(out.contains("b.txt"));
    }

    #[test]
    fn glob_returns_none_when_empty() {
        let dir = tempfile::tempdir().unwrap();
        let ctx =
            ToolContext::from_working_dir(dir.path().to_path_buf(), crate::fs_mode::FsMode::Write);
        let args = json!({"pat": "*.txt"});

        let out = tool_glob(&args, &ctx).unwrap();
        assert_eq!(out, "none");
    }
}
