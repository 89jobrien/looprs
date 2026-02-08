use super::error::ToolError;
use super::ToolContext;
use regex::Regex;
use serde_json::Value;
use std::fs;

use crate::config::MAX_GREP_HITS;

pub(super) fn tool_grep(args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
    let pat_str = args["pat"]
        .as_str()
        .ok_or(ToolError::MissingParameter("pat"))?;
    let path_prefix = args["path"].as_str().unwrap_or(".");

    let re = Regex::new(pat_str)?;
    let base = ctx.resolve_path(path_prefix);
    let glob_pattern = base.join("**/*");
    let pattern_str = glob_pattern
        .to_str()
        .ok_or(ToolError::MissingParameter("Invalid path"))?;

    let mut hits = Vec::new();

    for entry in glob::glob(pattern_str)?.filter_map(Result::ok) {
        if !entry.is_file() {
            continue;
        }

        let Ok(content) = fs::read_to_string(&entry) else {
            continue;
        };

        for (i, line) in content.lines().enumerate() {
            if re.is_match(line) {
                hits.push(format!("{}:{}: {}", entry.display(), i + 1, line.trim()));
                if hits.len() >= MAX_GREP_HITS {
                    break;
                }
            }
        }

        if hits.len() >= MAX_GREP_HITS {
            break;
        }
    }

    if hits.is_empty() {
        Ok("none".to_string())
    } else {
        Ok(hits.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    #[test]
    fn grep_finds_matches() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, "hello\nmatch me\n").unwrap();

        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
        };
        let args = json!({"pat": "match"});

        let out = tool_grep(&args, &ctx).unwrap();
        assert!(out.contains("match me"));
    }
}
