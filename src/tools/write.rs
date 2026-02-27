use super::ToolArgs;
use super::ToolContext;
use super::error::ToolError;
use serde_json::Value;
use std::fs;

pub(super) fn tool_write(args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
    let args = ToolArgs::new(args);
    let path = args.get_str("path")?;
    let content = args.get_str("content")?;

    let full_path = ctx.resolve_path(path)?;

    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&full_path, content)?;
    Ok("ok".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    #[test]
    fn write_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let ctx =
            ToolContext::from_working_dir(dir.path().to_path_buf(), crate::fs_mode::FsMode::Write);

        let args = json!({"path": "out.txt", "content": "hello"});
        let out = tool_write(&args, &ctx).unwrap();
        assert_eq!(out, "ok");

        let file = dir.path().join("out.txt");
        let content = fs::read_to_string(file).unwrap();
        assert_eq!(content, "hello");
    }

    #[test]
    fn write_blocks_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let ctx =
            ToolContext::from_working_dir(dir.path().to_path_buf(), crate::fs_mode::FsMode::Write);

        let args = json!({"path": "../escape.txt", "content": "nope"});
        let err = tool_write(&args, &ctx).unwrap_err();
        match err {
            ToolError::PathOutsideWorkingDir(_) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
