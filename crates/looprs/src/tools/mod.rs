mod availability;
mod bash;
mod edit;
pub mod error;
pub mod executor;
mod glob;
mod grep;
mod nu;
mod read;
mod write;

pub use executor::{DefaultToolExecutor, ToolExecutor};

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

    fn get_optional<T>(
        &self,
        key: &str,
        extract: fn(&Value) -> Option<T>,
        expected: &'static str,
    ) -> Result<Option<T>, ToolError> {
        let map = self.object()?;
        match map.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(value) => {
                extract(value)
                    .map(Some)
                    .ok_or_else(|| ToolError::InvalidParameterType {
                        key: key.to_string(),
                        expected,
                    })
            }
        }
    }

    pub fn get_str_optional(&self, key: &str) -> Result<Option<&str>, ToolError> {
        // Can't use get_optional due to lifetime constraints on as_str()
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
        self.get_optional(key, Value::as_u64, "u64")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    Read,
    Write,
    Edit,
    Glob,
    Grep,
    Nu,
    Bash,
}

impl Tool {
    const ALL: [Tool; 7] = [
        Tool::Read,
        Tool::Write,
        Tool::Edit,
        Tool::Glob,
        Tool::Grep,
        Tool::Nu,
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
            Tool::Nu => "nu",
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
            "nu" | "nushell" => Some(Tool::Nu),
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
            Tool::Nu => ToolDefinition {
                name: "nu".into(),
                description: "Execute a Nushell command. Returns stdout and stderr.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "cmd": {
                            "type": "string",
                            "description": "Nushell command to execute"
                        }
                    },
                    "required": ["cmd"]
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
                description: "Execute a Bash command. Returns stdout and stderr.".into(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "cmd": {
                            "type": "string",
                            "description": "Bash command to execute"
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
            Tool::Nu => nu::tool_nu(args),
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

// qual:allow(iosp) reason: "I/O boundary — validates filesystem mode before tool execution"
fn enforce_fs_mode(tool: Tool, args: &Value, ctx: &ToolContext) -> Result<(), ToolError> {
    let mode = ctx.fs_mode();
    match mode {
        FsMode::Write => Ok(()),
        FsMode::Read => match tool {
            Tool::Write | Tool::Edit | Tool::Nu | Tool::Bash => Err(ToolError::ModeDenied {
                tool: tool.name().to_string(),
                mode: mode.as_str().to_string(),
                reason: "file writes are disabled".to_string(),
            }),
            _ => Ok(()),
        },
        FsMode::Update => match tool {
            Tool::Nu | Tool::Bash => Err(ToolError::ModeDenied {
                tool: tool.name().to_string(),
                mode: mode.as_str().to_string(),
                reason: "shell commands are disabled (they can create/modify files)".to_string(),
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

/// Discover tool definitions from an MCP server at `server_url` via HTTP transport.
///
/// Sends a JSON-RPC `tools/list` request and maps each MCP tool into a
/// `ToolDefinition`. The caller is responsible for merging the result into
/// `get_tool_definitions()` so the LLM sees external tools alongside builtins.
pub async fn mcp_tool_definitions(server_url: &str) -> anyhow::Result<Vec<ToolDefinition>> {
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/list",
        "params": {}
    });

    let resp = client
        .post(server_url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    parse_mcp_tools_response(&resp)
}

/// Execute a named tool on an MCP server at `server_url` via HTTP transport.
///
/// Sends a JSON-RPC `tools/call` request with `name` and `arguments`, and
/// returns the tool result as a String. The caller is responsible for providing
/// a valid tool name that the server advertises via `mcp_tool_definitions`.
///
/// # Errors
/// Returns an error if the HTTP request fails, the server returns an error
/// response, or the result cannot be parsed.
pub async fn mcp_tool_call(
    server_url: &str,
    tool_name: &str,
    arguments: serde_json::Value,
) -> anyhow::Result<String> {
    let client = reqwest::Client::new();

    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": arguments,
        }
    });

    let resp = client
        .post(server_url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json::<serde_json::Value>()
        .await?;

    // JSON-RPC error
    if let Some(err) = resp.get("error") {
        anyhow::bail!("MCP tool call error: {err}");
    }

    parse_mcp_tool_call_response(&resp)
}

fn parse_mcp_tool_call_response(resp: &serde_json::Value) -> anyhow::Result<String> {
    // MCP tools/call result: { result: { content: [{ type: "text", text: "..." }] } }
    let content = resp
        .pointer("/result/content")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("MCP call response missing result.content array"))?;

    let text = content
        .iter()
        .filter_map(|item| {
            if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                item.get("text").and_then(|t| t.as_str()).map(str::to_owned)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(text)
}

fn parse_mcp_tools_response(resp: &serde_json::Value) -> anyhow::Result<Vec<ToolDefinition>> {
    let tools = resp
        .pointer("/result/tools")
        .and_then(|v| v.as_array())
        .ok_or_else(|| anyhow::anyhow!("MCP response missing result.tools array"))?;

    let defs = tools
        .iter()
        .filter_map(|t| {
            let name = t.get("name")?.as_str()?.to_owned();
            let description = t
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_owned();
            let input_schema = t
                .get("inputSchema")
                .cloned()
                .unwrap_or_else(|| serde_json::json!({"type": "object", "properties": {}}));
            Some(ToolDefinition {
                name,
                description,
                input_schema,
            })
        })
        .collect();

    Ok(defs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_mode::FsMode;
    use proptest::prelude::*;
    use std::io;

    // ── ToolError tests ─────────────────────────────────────────────────

    #[test]
    fn error_file_not_found_display() {
        let err = error::ToolError::FileNotFound("test.txt".to_string());
        assert_eq!(err.to_string(), "File not found: test.txt");
    }

    #[test]
    fn error_pattern_not_found_display() {
        let err = error::ToolError::PatternNotFound("pattern".to_string());
        assert_eq!(err.to_string(), "Pattern 'pattern' not found in file");
    }

    #[test]
    fn error_ambiguous_pattern_display() {
        let err = error::ToolError::AmbiguousPattern(3);
        assert_eq!(
            err.to_string(),
            "Pattern appears 3 times; use all=true or be more specific"
        );
    }

    #[test]
    fn error_missing_parameter_display() {
        let err = error::ToolError::MissingParameter("timeout".to_string());
        assert_eq!(err.to_string(), "Missing required parameter: timeout");
    }

    #[test]
    fn tool_error_invalid_parameter_type_display() {
        let err = error::ToolError::InvalidParameterType {
            key: "max_size".to_string(),
            expected: "u32",
        };
        assert_eq!(
            err.to_string(),
            "Invalid parameter type for max_size: expected u32"
        );
    }

    #[test]
    fn error_unknown_tool_display() {
        let err = error::ToolError::UnknownTool("fake_tool".to_string());
        assert_eq!(err.to_string(), "Unknown tool: fake_tool");
    }

    #[test]
    fn tool_error_mode_denied_display() {
        let err = error::ToolError::ModeDenied {
            tool: "system".to_string(),
            mode: "sandbox".to_string(),
            reason: "security policy".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Tool 'system' is not allowed in sandbox mode: security policy"
        );
    }

    #[test]
    fn error_command_failed_display() {
        let err = error::ToolError::CommandFailed("exit code 1".to_string());
        assert_eq!(err.to_string(), "Command execution failed: exit code 1");
    }

    #[test]
    fn error_path_outside_working_dir_display() {
        let err = error::ToolError::PathOutsideWorkingDir("../../../etc/passwd".to_string());
        assert_eq!(
            err.to_string(),
            "Path escapes working directory: ../../../etc/passwd"
        );
    }

    #[test]
    fn error_invalid_path_display() {
        let err = error::ToolError::InvalidPath("/invalid\0path".to_string());
        assert_eq!(err.to_string(), "Invalid path: /invalid\0path");
    }

    #[test]
    fn error_from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err: error::ToolError = io_err.into();

        let display = err.to_string();
        assert!(display.starts_with("IO error:"));
        assert!(display.contains("file not found"));
    }

    #[test]
    fn tool_error_from_regex_error() {
        let regex_err = regex::Error::Syntax("invalid regex".to_string());
        let err: error::ToolError = regex_err.into();

        let display = err.to_string();
        assert!(display.starts_with("Regex error:"));
    }

    #[test]
    fn error_from_glob_pattern_error() {
        // Test that GlobPattern variant displays correctly.
        // We test the From implementation by verifying the variant's display.
        let glob_err = ::glob::Pattern::new("[invalid").unwrap_err();
        let err: error::ToolError = glob_err.into();
        let display = err.to_string();
        assert!(display.starts_with("Glob pattern error:"));
    }

    // ── Property tests ──────────────────────────────────────────────────

    proptest! {
        #[test]
        fn get_str_never_panics(key in "[a-z]{1,10}", val in "\\PC{0,50}") {
            let args = serde_json::json!({ key.clone(): val });
            let ta = ToolArgs::new(&args);
            let _ = ta.get_str(&key);
        }

        #[test]
        fn get_str_optional_none_on_missing(key in "[a-z]{1,10}") {
            let args = serde_json::json!({});
            let ta = ToolArgs::new(&args);
            let result = ta.get_str_optional(&key).unwrap();
            prop_assert!(result.is_none());
        }

        #[test]
        fn get_u64_never_panics(key in "[a-z]{1,10}", val in proptest::num::u64::ANY) {
            let args = serde_json::json!({ key.clone(): val });
            let ta = ToolArgs::new(&args);
            let _ = ta.get_u64(&key);
        }

        #[test]
        fn get_bool_defaults_correctly(key in "[a-z]{1,10}") {
            let args = serde_json::json!({});
            let ta = ToolArgs::new(&args);
            prop_assert_eq!(ta.get_bool(&key, true), true);
            prop_assert_eq!(ta.get_bool(&key, false), false);
        }

        #[test]
        fn normalize_relative_rejects_parent_escape(
            segments in prop::collection::vec("[a-z]{1,5}", 1..5)
        ) {
            // A path that goes up more than it goes down should fail
            let mut path = String::from("../");
            for seg in &segments {
                path.push_str(seg);
                path.push('/');
            }
            let p = std::path::Path::new(&path);
            let result = normalize_relative(p);
            // Either fails (parent escape) or succeeds with a safe path
            if let Ok(normalized) = &result {
                // Must never contain ".."
                prop_assert!(
                    !normalized.to_string_lossy().contains(".."),
                    "normalized path contains '..': {:?}", normalized
                );
            }
        }
    }

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
    fn read_mode_blocks_write_edit_and_shells() {
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
        let err = execute_tool("nu", &args, &ctx).unwrap_err();
        assert!(matches!(err, ToolError::ModeDenied { .. }));

        let args = serde_json::json!({"cmd": "echo hi"});
        let err = execute_tool("bash", &args, &ctx).unwrap_err();
        assert!(matches!(err, ToolError::ModeDenied { .. }));
    }

    #[test]
    fn parse_mcp_tool_call_response_extracts_text() {
        let resp = serde_json::json!({
            "result": {
                "content": [
                    {"type": "text", "text": "hello"},
                    {"type": "image", "data": "..."},
                    {"type": "text", "text": "world"}
                ]
            }
        });
        let result = super::parse_mcp_tool_call_response(&resp).unwrap();
        assert_eq!(result, "hello\nworld");
    }

    #[test]
    fn parse_mcp_tool_call_response_errors_on_missing_content() {
        let resp = serde_json::json!({"result": {}});
        assert!(super::parse_mcp_tool_call_response(&resp).is_err());
    }

    #[test]
    fn mcp_tools_response_parses_correctly() {
        let resp = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "tools": [
                    {
                        "name": "shell",
                        "description": "Run a shell command",
                        "inputSchema": {
                            "type": "object",
                            "properties": { "cmd": { "type": "string" } },
                            "required": ["cmd"]
                        }
                    },
                    {
                        "name": "fetch",
                        "description": "Fetch a URL"
                    }
                ]
            }
        });

        let defs = parse_mcp_tools_response(&resp).unwrap();
        assert_eq!(defs.len(), 2);
        assert_eq!(defs[0].name, "shell");
        assert_eq!(defs[0].description, "Run a shell command");
        assert_eq!(defs[1].name, "fetch");
        assert_eq!(
            defs[1].input_schema,
            serde_json::json!({"type": "object", "properties": {}})
        );
    }

    #[test]
    fn mcp_tools_response_error_on_missing_tools() {
        let resp = serde_json::json!({"jsonrpc": "2.0", "id": 1, "result": {}});
        assert!(parse_mcp_tools_response(&resp).is_err());
    }

    #[test]
    fn exposes_nushell_and_bash_tools() {
        assert_eq!(Tool::from_name("nu"), Some(Tool::Nu));
        assert_eq!(Tool::from_name("nushell"), Some(Tool::Nu));
        assert_eq!(Tool::from_name("bash"), Some(Tool::Bash));

        let names = get_tool_definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();
        assert!(names.contains(&"nu".to_string()));
        assert!(names.contains(&"bash".to_string()));
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
