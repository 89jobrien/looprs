//! UserOutput port — abstraction over user-facing terminal/UI output.

/// Port: emit structured output to the user.
///
/// Implementations may render to a terminal, a log file, a TUI widget,
/// or a machine-readable JSON stream.
pub trait UserOutput: Send + Sync {
    /// Emit an informational message.
    fn info(&self, msg: &str);
    /// Emit a warning message.
    fn warn(&self, msg: &str);
    /// Emit an error message.
    fn error(&self, msg: &str);
    /// Emit assistant-generated text.
    fn assistant_text(&self, text: &str);
    /// Emit a tool-call banner with an input preview.
    fn tool_call(&self, tool_name: &str, input_preview: &str);
    /// Emit a successful tool-completion marker.
    fn tool_ok(&self);
    /// Emit a failed tool-completion marker.
    fn tool_err(&self, err_msg: &str);

    /// Emit a single streaming chunk of assistant text.
    ///
    /// Called once per token/chunk during streaming inference. The default
    /// implementation delegates to `assistant_text`, so existing adapters
    /// remain valid until they opt into incremental rendering.
    fn write_chunk(&self, chunk: &str) {
        self.assistant_text(chunk);
    }
}
