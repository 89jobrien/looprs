use thiserror::Error;

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("File not found: {0}")]
    FileNotFound(String),

    #[error("Pattern '{0}' not found in file")]
    PatternNotFound(String),

    #[error("Pattern appears {0} times; use all=true or be more specific")]
    AmbiguousPattern(usize),

    #[error("Missing required parameter: {0}")]
    MissingParameter(String),

    #[error("Invalid parameter type for {key}: expected {expected}")]
    InvalidParameterType { key: String, expected: &'static str },

    #[error("Unknown tool: {0}")]
    UnknownTool(String),

    #[error("Tool '{tool}' is not allowed in {mode} mode: {reason}")]
    ModeDenied {
        tool: String,
        mode: String,
        reason: String,
    },

    #[error("Command execution failed: {0}")]
    CommandFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Glob pattern error: {0}")]
    GlobPattern(#[from] glob::PatternError),

    #[error("Path escapes working directory: {0}")]
    PathOutsideWorkingDir(String),

    #[error("Invalid path: {0}")]
    InvalidPath(String),
}
