use super::ToolArgs;
use super::ToolContext;
use super::error::ToolError;
use serde_json::Value;
use std::fmt::Write as _;
use std::fs;
use std::io::{BufRead, BufReader};

pub(super) fn tool_read(args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
    let args = ToolArgs::new(args);
    let path = args.get_str("path")?;
    let offset = args.get_u64("offset")?.unwrap_or(0) as usize;
    let limit = args.get_u64("limit")?;

    let full_path = ctx.resolve_path(path)?;

    let file = fs::File::open(&full_path).map_err(|_| ToolError::FileNotFound(path.to_string()))?;
    let mut reader = BufReader::new(file);
    let mut line = String::new();

    if limit == Some(0) {
        return Ok(String::new());
    }

    for _ in 0..offset {
        line.clear();
        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|_| ToolError::FileNotFound(path.to_string()))?;
        if bytes_read == 0 {
            return Ok("(EOF)".to_string());
        }
    }

    let mut output = String::new();
    let mut written = 0usize;
    loop {
        if let Some(limit) = limit
            && written >= limit as usize
        {
            break;
        }

        line.clear();
        let bytes_read = reader
            .read_line(&mut line)
            .map_err(|_| ToolError::FileNotFound(path.to_string()))?;
        if bytes_read == 0 {
            if written == 0 {
                return Ok("(EOF)".to_string());
            }
            break;
        }

        let trimmed = line.trim_end_matches(&['\n', '\r'][..]);

        let line_no = offset + written + 1;
        let _ = writeln!(&mut output, "{:4}| {}", line_no, trimmed);
        written += 1;
    }

    while output.ends_with('\n') {
        output.pop();
    }

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

        let ctx =
            ToolContext::from_working_dir(dir.path().to_path_buf(), crate::fs_mode::FsMode::Write);
        let args = json!({"path": "a.txt", "offset": 1, "limit": 1});

        let out = tool_read(&args, &ctx).unwrap();
        assert!(out.contains("2| b"));
    }

    #[test]
    fn read_blocks_path_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let ctx =
            ToolContext::from_working_dir(dir.path().to_path_buf(), crate::fs_mode::FsMode::Write);
        let args = json!({"path": "../escape.txt"});

        let err = tool_read(&args, &ctx).unwrap_err();
        match err {
            ToolError::PathOutsideWorkingDir(_) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
