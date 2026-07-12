use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum ToolError {
    #[error("File not found: {0}")]
    #[diagnostic(code(looprs::tool::file_not_found))]
    FileNotFound(String),

    #[error("Pattern '{0}' not found in file")]
    #[diagnostic(
        code(looprs::tool::pattern_not_found),
        help("Check the pattern spelling; patterns are case-sensitive by default")
    )]
    PatternNotFound(String),

    #[error("Pattern appears {0} times; use all=true or be more specific")]
    #[diagnostic(code(looprs::tool::ambiguous_pattern))]
    AmbiguousPattern(usize),

    #[error("Missing required parameter: {0}")]
    #[diagnostic(code(looprs::tool::missing_parameter))]
    MissingParameter(String),

    #[error("Invalid parameter type for {key}: expected {expected}")]
    #[diagnostic(code(looprs::tool::invalid_parameter_type))]
    InvalidParameterType { key: String, expected: &'static str },

    #[error("Unknown tool: {0}")]
    #[diagnostic(
        code(looprs::tool::unknown),
        help("Available tools: read, write, edit, glob, grep, nu, bash")
    )]
    UnknownTool(String),

    #[error("Tool '{tool}' is not allowed in {mode} mode: {reason}")]
    #[diagnostic(code(looprs::tool::mode_denied))]
    ModeDenied {
        tool: String,
        mode: String,
        reason: String,
    },

    #[error("Command execution failed: {0}")]
    #[diagnostic(code(looprs::tool::command_failed))]
    CommandFailed(String),

    #[error("IO error: {0}")]
    #[diagnostic(code(looprs::tool::io))]
    Io(#[from] std::io::Error),

    #[error("Regex error: {0}")]
    #[diagnostic(code(looprs::tool::regex))]
    Regex(#[from] regex::Error),

    #[error("Glob pattern error: {0}")]
    #[diagnostic(code(looprs::tool::glob_pattern))]
    GlobPattern(#[from] glob::PatternError),

    #[error("Path escapes working directory: {0}")]
    #[diagnostic(
        code(looprs::tool::path_outside_working_dir),
        help("Use relative paths that stay within the working directory")
    )]
    PathOutsideWorkingDir(String),

    #[error("Invalid path: {0}")]
    #[diagnostic(code(looprs::tool::invalid_path))]
    InvalidPath(String),
}
