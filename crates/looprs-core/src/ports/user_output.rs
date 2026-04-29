//! UserOutput port — abstraction over user-facing terminal/UI output.

/// Port: emit structured output to the user.
///
/// Implementations may render to a terminal, a log file, a TUI widget,
/// or a machine-readable JSON stream.
pub trait UserOutput: Send + Sync {
    fn info(&self, msg: &str);
    fn warn(&self, msg: &str);
    fn error(&self, msg: &str);
    fn assistant_text(&self, text: &str);
    fn tool_call(&self, tool_name: &str, input_preview: &str);
    fn tool_ok(&self);
    fn tool_err(&self, err_msg: &str);
}
