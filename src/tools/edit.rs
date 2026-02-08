use super::ToolContext;
use super::error::ToolError;
use serde_json::Value;
use std::fs;

pub(super) fn tool_edit(args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
    let path = args["path"]
        .as_str()
        .ok_or(ToolError::MissingParameter("path"))?;
    let old = args["old"]
        .as_str()
        .ok_or(ToolError::MissingParameter("old"))?;
    let new = args["new"].as_str().unwrap_or("");
    let all = args["all"].as_bool().unwrap_or(false);

    let full_path = ctx.resolve_path(path);
    let text =
        fs::read_to_string(&full_path).map_err(|_| ToolError::FileNotFound(path.to_string()))?;

    if !text.contains(old) {
        return Err(ToolError::PatternNotFound(old.to_string()));
    }

    let count = text.matches(old).count();
    if !all && count > 1 {
        return Err(ToolError::AmbiguousPattern(count));
    }

    let replacement = if all {
        text.replace(old, new)
    } else {
        text.replacen(old, new, 1)
    };

    fs::write(&full_path, replacement)?;
    Ok("ok".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    #[test]
    fn edit_replaces_text() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, "hello world").unwrap();

        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
        };
        let args = json!({"path": "a.txt", "old": "world", "new": "there"});

        let out = tool_edit(&args, &ctx).unwrap();
        assert_eq!(out, "ok");

        let content = fs::read_to_string(file).unwrap();
        assert_eq!(content, "hello there");
    }
}
