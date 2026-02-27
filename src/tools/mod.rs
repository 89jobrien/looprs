mod availability;
mod bash;
mod edit;
pub mod error;
mod glob;
mod grep;
mod read;
mod write;

use serde_json::{Value, json};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

use crate::fs_mode::FsMode;

use crate::api::ToolDefinition;
use crate::errors::ToolContextError;

pub use error::ToolError;

pub struct ToolContext {
    pub working_dir: PathBuf,
    fs_mode: Arc<AtomicU8>,
}

impl ToolContext {
    #[allow(dead_code)]
    pub fn new() -> Result<Self, ToolContextError> {
        Self::new_with_mode(FsMode::default())
    }

    pub fn new_with_mode(mode: FsMode) -> Result<Self, ToolContextError> {
        Ok(Self {
            working_dir: env::current_dir().map_err(ToolContextError::WorkingDirUnavailable)?,
            fs_mode: Arc::new(AtomicU8::new(mode.to_u8())),
        })
    }

    #[allow(dead_code)]
    pub fn from_working_dir(working_dir: PathBuf, mode: FsMode) -> Self {
        Self {
            working_dir,
            fs_mode: Arc::new(AtomicU8::new(mode.to_u8())),
        }
    }

    pub fn fs_mode(&self) -> FsMode {
        FsMode::from_u8(self.fs_mode.load(Ordering::Relaxed))
    }

    pub fn set_fs_mode(&self, mode: FsMode) {
        self.fs_mode.store(mode.to_u8(), Ordering::Relaxed);
    }

    pub fn fs_mode_handle(&self) -> Arc<AtomicU8> {
        self.fs_mode.clone()
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
        self.args
            .as_object()
            .ok_or_else(|| ToolError::InvalidParameterType {
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
        value
            .as_str()
            .ok_or_else(|| ToolError::InvalidParameterType {
                key: key.to_string(),
                expected: "string",
            })
    }

    pub fn get_str_optional(&self, key: &str) -> Result<Option<&str>, ToolError> {
        let map = self.object()?;
        match map.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => {
                value
                    .as_str()
                    .map(Some)
                    .ok_or_else(|| ToolError::InvalidParameterType {
                        key: key.to_string(),
                        expected: "string",
                    })
            }
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
            Some(value) => {
                value
                    .as_u64()
                    .map(Some)
                    .ok_or_else(|| ToolError::InvalidParameterType {
                        key: key.to_string(),
                        expected: "u64",
                    })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    Read,
    Write,
    Edit,
    Glob,
    Grep,
    Bash,
}

impl Tool {
    const ALL: [Tool; 6] = [
        Tool::Read,
        Tool::Write,
        Tool::Edit,
        Tool::Glob,
        Tool::Grep,
        Tool::Bash,
    ];

    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            Tool::Read => "read",
            Tool::Write => "write",
            Tool::Edit => "edit",
            Tool::Glob => "glob",
            Tool::Grep => "grep",
            Tool::Bash => "bash",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "read" => Some(Tool::Read),
            "write" => Some(Tool::Write),
            "edit" => Some(Tool::Edit),
            "glob" => Some(Tool::Glob),
            "grep" => Some(Tool::Grep),
            "bash" => Some(Tool::Bash),
            _ => None,
        }
    }

    pub fn definition(&self) -> ToolDefinition {
        match self {
            Tool::Read => ToolDefinition {
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
            Tool::Write => ToolDefinition {
                name: "write".into(),
                description:
                    "Write content to file (creates or overwrites). Parent directories are created if needed."
                        .into(),
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
            Tool::Edit => ToolDefinition {
                name: "edit".into(),
                description:
                    "Replace text in file. The 'old' string must be unique unless all=true is set."
                        .into(),
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
            Tool::Glob => ToolDefinition {
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
            Tool::Grep => ToolDefinition {
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
            Tool::Bash => ToolDefinition {
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
        }
    }

    pub fn execute(&self, args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
        match self {
            Tool::Read => read::tool_read(args, ctx),
            Tool::Write => write::tool_write(args, ctx),
            Tool::Edit => edit::tool_edit(args, ctx),
            Tool::Glob => glob::tool_glob(args, ctx),
            Tool::Grep => grep::tool_grep(args, ctx),
            Tool::Bash => bash::tool_bash(args),
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

fn enforce_fs_mode(tool: Tool, args: &Value, ctx: &ToolContext) -> Result<(), ToolError> {
    let mode = ctx.fs_mode();
    match mode {
        FsMode::Write => Ok(()),
        FsMode::Read => match tool {
            Tool::Write | Tool::Edit | Tool::Bash => Err(ToolError::ModeDenied {
                tool: tool.name().to_string(),
                mode: mode.as_str().to_string(),
                reason: "file writes are disabled".to_string(),
            }),
            _ => Ok(()),
        },
        FsMode::Update => match tool {
            Tool::Bash => Err(ToolError::ModeDenied {
                tool: tool.name().to_string(),
                mode: mode.as_str().to_string(),
                reason: "bash is disabled (it can create/modify files)".to_string(),
            }),
            Tool::Edit => Ok(()),
            Tool::Write => {
                let tool_args = ToolArgs::new(args);
                let path = tool_args.get_str("path")?;
                let full_path = ctx.resolve_path(path)?;
                if full_path.is_file() {
                    Ok(())
                } else {
                    Err(ToolError::ModeDenied {
                        tool: tool.name().to_string(),
                        mode: mode.as_str().to_string(),
                        reason: "cannot create new files in update mode".to_string(),
                    })
                }
            }
            _ => Ok(()),
        },
    }
}

pub fn execute_tool(name: &str, args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
    match Tool::from_name(name) {
        Some(tool) => {
            enforce_fs_mode(tool, args, ctx)?;
            tool.execute(args, ctx)
        }
        None => Err(ToolError::UnknownTool(name.to_string())),
    }
}

pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    Tool::ALL.iter().map(|tool| tool.definition()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_mode::FsMode;

    #[test]
    fn resolve_path_blocks_absolute_paths() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolContext::from_working_dir(dir.path().to_path_buf(), FsMode::Write);

        let err = ctx.resolve_path("/etc/passwd").unwrap_err();
        match err {
            ToolError::PathOutsideWorkingDir(_) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn resolve_path_blocks_parent_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolContext::from_working_dir(dir.path().to_path_buf(), FsMode::Write);

        let err = ctx.resolve_path("../escape.txt").unwrap_err();
        match err {
            ToolError::PathOutsideWorkingDir(_) => {}
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn read_mode_blocks_write_edit_and_bash() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolContext::from_working_dir(dir.path().to_path_buf(), FsMode::Read);

        let args = serde_json::json!({"path": "out.txt", "content": "hello"});
        let err = execute_tool("write", &args, &ctx).unwrap_err();
        assert!(matches!(err, ToolError::ModeDenied { .. }));

        let file = dir.path().join("a.txt");
        std::fs::write(&file, "hello world").unwrap();
        let args = serde_json::json!({"path": "a.txt", "old": "world", "new": "there"});
        let err = execute_tool("edit", &args, &ctx).unwrap_err();
        assert!(matches!(err, ToolError::ModeDenied { .. }));

        let args = serde_json::json!({"cmd": "echo hi"});
        let err = execute_tool("bash", &args, &ctx).unwrap_err();
        assert!(matches!(err, ToolError::ModeDenied { .. }));
    }

    #[test]
    fn update_mode_blocks_new_file_but_allows_existing_file_write() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = ToolContext::from_working_dir(dir.path().to_path_buf(), FsMode::Update);

        let args = serde_json::json!({"path": "new.txt", "content": "hello"});
        let err = execute_tool("write", &args, &ctx).unwrap_err();
        assert!(matches!(err, ToolError::ModeDenied { .. }));

        std::fs::write(dir.path().join("existing.txt"), "old").unwrap();
        let args = serde_json::json!({"path": "existing.txt", "content": "new"});
        let out = execute_tool("write", &args, &ctx).unwrap();
        assert_eq!(out, "ok");
        let content = std::fs::read_to_string(dir.path().join("existing.txt")).unwrap();
        assert_eq!(content, "new");
    }

    #[test]
    fn update_mode_allows_edit_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "hello world").unwrap();
        let ctx = ToolContext::from_working_dir(dir.path().to_path_buf(), FsMode::Update);

        let args = serde_json::json!({"path": "a.txt", "old": "world", "new": "there"});
        let out = execute_tool("edit", &args, &ctx).unwrap();
        assert_eq!(out, "ok");
        let content = std::fs::read_to_string(dir.path().join("a.txt")).unwrap();
        assert_eq!(content, "hello there");
    }
}
