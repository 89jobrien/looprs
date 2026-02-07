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
    MissingParameter(&'static str),

    #[error("Command execution failed: {0}")]
    CommandFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("Glob pattern error: {0}")]
    GlobPattern(#[from] glob::PatternError),
}
