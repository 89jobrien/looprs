use anyhow::Result;
use regex::Regex;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::api::ToolDefinition;
use crate::config::MAX_GREP_HITS;
use crate::errors::ToolError;

pub struct ToolContext {
    pub working_dir: PathBuf,
}

impl ToolContext {
    pub fn new() -> Result<Self> {
        Ok(Self {
            working_dir: env::current_dir()?,
        })
    }

    pub fn resolve_path(&self, path: &str) -> PathBuf {
        let p = Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.working_dir.join(p)
        }
    }
}

pub fn execute_tool(name: &str, args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
    match name {
        "read" => tool_read(args, ctx),
        "write" => tool_write(args, ctx),
        "edit" => tool_edit(args, ctx),
        "glob" => tool_glob(args, ctx),
        "grep" => tool_grep(args, ctx),
        "bash" => tool_bash(args),
        _ => Err(ToolError::MissingParameter("Unknown tool")),
    }
}

fn tool_read(args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
    let path = args["path"]
        .as_str()
        .ok_or(ToolError::MissingParameter("path"))?;
    let offset = args["offset"].as_u64().unwrap_or(0) as usize;
    let limit = args["limit"].as_u64();

    let full_path = ctx.resolve_path(path);
    let content = fs::read_to_string(&full_path)
        .map_err(|_| ToolError::FileNotFound(path.to_string()))?;

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

fn tool_write(args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
    let path = args["path"]
        .as_str()
        .ok_or(ToolError::MissingParameter("path"))?;
    let content = args["content"].as_str().unwrap_or("");

    let full_path = ctx.resolve_path(path);

    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&full_path, content)?;
    Ok("ok".to_string())
}

fn tool_edit(args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
    let path = args["path"]
        .as_str()
        .ok_or(ToolError::MissingParameter("path"))?;
    let old = args["old"]
        .as_str()
        .ok_or(ToolError::MissingParameter("old"))?;
    let new = args["new"].as_str().unwrap_or("");
    let all = args["all"].as_bool().unwrap_or(false);

    let full_path = ctx.resolve_path(path);
    let text = fs::read_to_string(&full_path)
        .map_err(|_| ToolError::FileNotFound(path.to_string()))?;

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

fn tool_glob(args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
    let pattern = args["pat"]
        .as_str()
        .ok_or(ToolError::MissingParameter("pat"))?;
    let path_prefix = args["path"].as_str().unwrap_or(".");

    let base = ctx.resolve_path(path_prefix);
    let full_pattern = base.join(pattern);
    let pattern_str = full_pattern
        .to_str()
        .ok_or(ToolError::MissingParameter("Invalid path"))?;

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

fn tool_grep(args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
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

fn tool_bash(args: &Value) -> Result<String, ToolError> {
    let cmd = args["cmd"]
        .as_str()
        .ok_or(ToolError::MissingParameter("cmd"))?;

    let output = Command::new("bash")
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    let mut result = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.stderr.is_empty() {
        result.push_str("\n--- stderr ---\n");
        result.push_str(&String::from_utf8_lossy(&output.stderr));
    }

    if !output.status.success() {
        return Err(ToolError::CommandFailed(format!(
            "Exit code: {}",
            output.status.code().unwrap_or(-1)
        )));
    }

    Ok(if result.trim().is_empty() {
        "(empty)".to_string()
    } else {
        result
    })
}

pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "read".into(),
            description: "Read file with line numbers. Supports offset and limit for pagination."
                .into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to read"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line number to start from (0-indexed)",
                        "default": 0
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read"
                    }
                },
                "required": ["path"]
            }),
        },
        ToolDefinition {
            name: "write".into(),
            description: "Write content to file (creates or overwrites). Parent directories are created if needed.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
        },
        ToolDefinition {
            name: "edit".into(),
            description: "Replace text in file. The 'old' string must be unique unless all=true is set.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string" },
                    "old": {
                        "type": "string",
                        "description": "Exact text to find and replace"
                    },
                    "new": {
                        "type": "string",
                        "description": "Replacement text"
                    },
                    "all": {
                        "type": "boolean",
                        "description": "Replace all occurrences (default: false)",
                        "default": false
                    }
                },
                "required": ["path", "old", "new"]
            }),
        },
        ToolDefinition {
            name: "glob".into(),
            description: "Find files matching glob pattern. Results sorted by modification time (newest first).".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pat": {
                        "type": "string",
                        "description": "Glob pattern (e.g., '*.rs', '**/*.toml')"
                    },
                    "path": {
                        "type": "string",
                        "description": "Base directory for search (default: current directory)",
                        "default": "."
                    }
                },
                "required": ["pat"]
            }),
        },
        ToolDefinition {
            name: "grep".into(),
            description: "Search files for regex pattern. Returns up to 50 matches.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pat": {
                        "type": "string",
                        "description": "Regex pattern to search for"
                    },
                    "path": {
                        "type": "string",
                        "description": "Base directory for search (default: current directory)",
                        "default": "."
                    }
                },
                "required": ["pat"]
            }),
        },
        ToolDefinition {
            name: "bash".into(),
            description: "Execute shell command. Returns stdout and stderr.".into(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "cmd": {
                        "type": "string",
                        "description": "Shell command to execute"
                    }
                },
                "required": ["cmd"]
            }),
        },
    ]
}
