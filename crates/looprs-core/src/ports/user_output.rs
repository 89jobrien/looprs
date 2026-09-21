//! UserOutput port — abstraction over user-facing terminal/UI output.

// Remaining: streaming write_chunk() path (idea #5, blocked on provider streaming support).

/// Port: emit structured output to the user.
///
/// Implementations may render to a terminal, a log file, a TUI widget,
/// or a machine-readable JSON stream.
pub trait UserOutput: Send + Sync {
    /// Emits an informational message.
    fn info(&self, msg: &str);
    /// Emits a warning message.
    fn warn(&self, msg: &str);
    /// Emits an error message.
    fn error(&self, msg: &str);
    /// Emits assistant-generated text.
    fn assistant_text(&self, text: &str);
    /// Reports a tool call with a preview of its input.
    fn tool_call(&self, tool_name: &str, input_preview: &str);
    /// Reports successful completion of the current tool call.
    fn tool_ok(&self);
    /// Reports failure of the current tool call.
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
