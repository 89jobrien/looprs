pub mod error;
mod availability;
mod bash;
mod edit;
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
        "read" => read::tool_read(args, ctx),
        "write" => write::tool_write(args, ctx),
        "edit" => edit::tool_edit(args, ctx),
        "glob" => glob::tool_glob(args, ctx),
        "grep" => grep::tool_grep(args, ctx),
        "bash" => bash::tool_bash(args),
        _ => Err(ToolError::MissingParameter("Unknown tool")),
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
