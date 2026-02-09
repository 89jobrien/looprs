use super::ToolArgs;
use super::ToolContext;
use super::error::ToolError;
use serde_json::Value;
use std::fs;

pub(super) fn tool_read(args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
    let args = ToolArgs::new(args);
    let path = args.get_str("path")?;
    let offset = args.get_u64("offset")?.unwrap_or(0) as usize;
    let limit = args.get_u64("limit")?;

    let full_path = ctx.resolve_path(path)?;
    let content =
        fs::read_to_string(&full_path).map_err(|_| ToolError::FileNotFound(path.to_string()))?;

    let lines: Vec<&str> = content.lines().collect();

    if offset >= lines.len() {
        return Ok("(EOF)".to_string());
    }

    let end = limit
        .map(|l| (offset + l as usize).min(lines.len()))
        .unwrap_or(lines.len());

    let output = lines[offset..end]
        .iter()
        .enumerate()
        .map(|(i, line)| format!("{:4}| {}", offset + i + 1, line))
        .collect::<Vec<_>>()
        .join("\n");

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    #[test]
    fn read_respects_offset_and_limit() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, "a\nb\nc\n").unwrap();

        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
        };
        let args = json!({"path": "a.txt", "offset": 1, "limit": 1});

        let out = tool_read(&args, &ctx).unwrap();
        assert!(out.contains("2| b"));
    }

    #[test]
    fn read_blocks_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
        };
        let args = json!({"path": "../escape.txt"});

        let err = tool_read(&args, &ctx).unwrap_err();
        match err {
            ToolError::PathOutsideWorkingDir(_) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
