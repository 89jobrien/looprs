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

#[cfg(test)]
mod tests {
    use super::*;

    fn code_of(err: &ToolError) -> String {
        err.code()
            .expect("diagnostic must carry a code")
            .to_string()
    }

    #[test]
    fn display_messages_are_actionable() {
        assert_eq!(
            ToolError::FileNotFound("/tmp/x.rs".into()).to_string(),
            "File not found: /tmp/x.rs"
        );
        assert_eq!(
            ToolError::AmbiguousPattern(3).to_string(),
            "Pattern appears 3 times; use all=true or be more specific"
        );
        assert_eq!(
            ToolError::InvalidParameterType {
                key: "all".into(),
                expected: "bool"
            }
            .to_string(),
            "Invalid parameter type for all: expected bool"
        );
        assert_eq!(
            ToolError::ModeDenied {
                tool: "write".into(),
                mode: "read-only".into(),
                reason: "fs_mode".into()
            }
            .to_string(),
            "Tool 'write' is not allowed in read-only mode: fs_mode"
        );
    }

    #[test]
    fn diagnostic_codes_cover_variants() {
        let cases: Vec<(ToolError, &str)> = vec![
            (
                ToolError::FileNotFound("f".into()),
                "looprs::tool::file_not_found",
            ),
            (
                ToolError::PatternNotFound("p".into()),
                "looprs::tool::pattern_not_found",
            ),
            (
                ToolError::MissingParameter("q".into()),
                "looprs::tool::missing_parameter",
            ),
            (ToolError::UnknownTool("zz".into()), "looprs::tool::unknown"),
            (
                ToolError::CommandFailed("c".into()),
                "looprs::tool::command_failed",
            ),
            (
                ToolError::PathOutsideWorkingDir("../x".into()),
                "looprs::tool::path_outside_working_dir",
            ),
            (
                ToolError::InvalidPath("\0".into()),
                "looprs::tool::invalid_path",
            ),
        ];
        for (err, expected) in cases {
            assert_eq!(code_of(&err), expected, "variant: {err}");
        }
    }

    #[test]
    fn help_text_present_where_guidance_exists() {
        assert!(ToolError::PatternNotFound("p".into()).help().is_some());
        assert!(ToolError::UnknownTool("z".into()).help().is_some());
        assert!(
            ToolError::PathOutsideWorkingDir("../x".into())
                .help()
                .is_some()
        );
    }

    #[test]
    fn io_error_converts_via_from() {
        let io = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err: ToolError = io.into();
        assert!(matches!(err, ToolError::Io(_)));
        assert_eq!(code_of(&err), "looprs::tool::io");
        assert!(err.to_string().starts_with("IO error:"));
    }
}
