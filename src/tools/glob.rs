use super::ToolArgs;
use super::ToolContext;
use super::error::ToolError;
use crate::config::{MAX_GLOB_HITS, MAX_GLOB_OUTPUT_CHARS};
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
        return Ok("none".to_string());
    }

    let mut omitted_entries = 0usize;
    if paths.len() > MAX_GLOB_HITS {
        omitted_entries += paths.len() - MAX_GLOB_HITS;
        paths.truncate(MAX_GLOB_HITS);
    }

    let mut output = String::new();
    let mut truncated_by_chars = false;

    for (index, path) in paths.iter().enumerate() {
        let line = path.display().to_string();
        let line_chars = line.chars().count();
        let separator_chars = if output.is_empty() { 0 } else { 1 };

        if output.chars().count() + separator_chars + line_chars > MAX_GLOB_OUTPUT_CHARS {
            omitted_entries += paths.len() - index;
            truncated_by_chars = true;
            break;
        }

        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&line);
    }

    if omitted_entries > 0 {
        if !output.is_empty() {
            output.push('\n');
        }

        if truncated_by_chars {
            output.push_str(&format!(
                "[truncated glob results: {} entries omitted due to size cap]",
                omitted_entries
            ));
        } else {
            output.push_str(&format!(
                "[truncated glob results: {} entries omitted due to hit cap]",
                omitted_entries
            ));
        }
    }

    Ok(output)
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

    #[test]
    fn glob_caps_large_result_sets() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..(crate::config::MAX_GLOB_HITS + 10) {
            let p = dir.path().join(format!("f{i:04}.txt"));
            fs::write(p, "x").unwrap();
        }

        let ctx =
            ToolContext::from_working_dir(dir.path().to_path_buf(), crate::fs_mode::FsMode::Write);
        let args = json!({"pat": "*.txt"});

        let out = tool_glob(&args, &ctx).unwrap();
        assert!(out.contains("[truncated glob results:"));
        assert!(out.contains("omitted"));
    }
}
