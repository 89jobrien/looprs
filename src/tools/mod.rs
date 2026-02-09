mod availability;
mod bash;
mod edit;
pub mod error;
mod glob;
mod grep;
mod read;
mod write;

use anyhow::Result;
use serde_json::{json, Value};
use std::env;
use std::path::{Path, PathBuf};

use crate::api::ToolDefinition;

pub use error::ToolError;

pub struct ToolContext {
    pub working_dir: PathBuf,
}

impl ToolContext {
    pub fn new() -> Result<Self> {
        Ok(Self {
            working_dir: env::current_dir()?,
        })
    }

    /// Resolve a user-provided path within the working directory.
    ///
    /// Security: this is a jail. Relative paths may not escape `working_dir`.
    /// Absolute paths are denied by default.
    pub fn resolve_path(&self, path: &str) -> Result<PathBuf, ToolError> {
        let p = Path::new(path);

        if p.is_absolute() {
            return Err(ToolError::PathOutsideWorkingDir(path.to_string()));
        }

        // Canonicalize base (exists) to get stable absolute prefix.
        let base = self.working_dir.canonicalize().map_err(ToolError::Io)?;

        let rel = normalize_relative(p)
            .map_err(|_| ToolError::PathOutsideWorkingDir(path.to_string()))?;
        let joined = base.join(rel);

        // If the target exists, canonicalize to defend against symlink escapes.
        // For non-existent targets (e.g., writes), we fall back to lexical checks.
        if let Ok(canon) = joined.canonicalize() {
            if !canon.starts_with(&base) {
                return Err(ToolError::PathOutsideWorkingDir(path.to_string()));
            }
            return Ok(canon);
        }

        if !joined.starts_with(&base) {
            return Err(ToolError::PathOutsideWorkingDir(path.to_string()));
        }

        Ok(joined)
    }
}

pub(crate) struct ToolArgs<'a> {
    args: &'a Value,
}

impl<'a> ToolArgs<'a> {
    pub fn new(args: &'a Value) -> Self {
        Self { args }
    }

    fn object(&self) -> Result<&serde_json::Map<String, Value>, ToolError> {
        self.args.as_object().ok_or_else(|| ToolError::InvalidParameterType {
            key: "<root>".to_string(),
            expected: "object",
        })
    }

    fn get_value(&self, key: &str) -> Result<&Value, ToolError> {
        let map = self.object()?;
        map.get(key)
            .ok_or_else(|| ToolError::MissingParameter(key.to_string()))
    }

    pub fn get_str(&self, key: &str) -> Result<&str, ToolError> {
        let value = self.get_value(key)?;
        value.as_str().ok_or_else(|| ToolError::InvalidParameterType {
            key: key.to_string(),
            expected: "string",
        })
    }

    pub fn get_str_optional(&self, key: &str) -> Result<Option<&str>, ToolError> {
        let map = self.object()?;
        match map.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => value.as_str().map(Some).ok_or_else(|| {
                ToolError::InvalidParameterType {
                    key: key.to_string(),
                    expected: "string",
                }
            }),
        }
    }

    pub fn get_bool(&self, key: &str, default: bool) -> bool {
        let map = match self.args.as_object() {
            Some(map) => map,
            None => return default,
        };
        match map.get(key) {
            None | Some(Value::Null) => default,
            Some(value) => value.as_bool().unwrap_or(default),
        }
    }

    pub fn get_u64(&self, key: &str) -> Result<Option<u64>, ToolError> {
        let map = self.object()?;
        match map.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => value.as_u64().map(Some).ok_or_else(|| {
                ToolError::InvalidParameterType {
                    key: key.to_string(),
                    expected: "u64",
                }
            }),
        }
    }
}

fn normalize_relative(p: &Path) -> Result<PathBuf, ()> {
    use std::path::Component;

    let mut out = PathBuf::new();

    for comp in p.components() {
        match comp {
            Component::CurDir => {}
            Component::Normal(part) => out.push(part),
            Component::ParentDir => {
                if !out.pop() {
                    return Err(());
                }
            }
            // Reject anything that would imply an absolute/anchored path.
            Component::RootDir | Component::Prefix(_) => return Err(()),
        }
    }

    Ok(out)
}

pub fn execute_tool(name: &str, args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
    match name {
        "read" => read::tool_read(args, ctx),
        "write" => write::tool_write(args, ctx),
        "edit" => edit::tool_edit(args, ctx),
        "glob" => glob::tool_glob(args, ctx),
        "grep" => grep::tool_grep(args, ctx),
        "bash" => bash::tool_bash(args),
        _ => Err(ToolError::UnknownTool(name.to_string())),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_path_blocks_absolute_paths() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
        };

        let err = ctx.resolve_path("/etc/passwd").unwrap_err();
        match err {
            ToolError::PathOutsideWorkingDir(_) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn resolve_path_blocks_parent_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolContext {
            working_dir: dir.path().to_path_buf(),
        };

        let err = ctx.resolve_path("../escape.txt").unwrap_err();
        match err {
            ToolError::PathOutsideWorkingDir(_) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
