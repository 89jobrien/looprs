//! A `UserOutput` port implementation that forwards every event over a
//! channel instead of writing to stdout, so the chat TUI can render
//! streamed assistant text inside its own ratatui frame in real time.

use looprs::ports::UserOutput;
use tokio::sync::mpsc::UnboundedSender;

/// A single rendering event emitted by the agent runtime.
///
/// Mirrors the [`UserOutput`] port methods one-to-one so the TUI can replay
/// them into a frame instead of writing to stdout. Apply them to a text
/// buffer with [`apply_output_event`].
#[derive(Debug, Clone)]
pub enum OutputEvent {
    /// A fragment of streamed assistant text, appended verbatim with no
    /// separator. Chunks are not line-delimited.
    Chunk(String),
    /// Informational notice, rendered on its own `[info]` line.
    Info(String),
    /// Warning notice, rendered on its own `[warn]` line.
    Warn(String),
    /// Error notice, rendered on its own `[error]` line.
    Error(String),
    /// A tool invocation is starting.
    ToolCall {
        /// Name of the tool being invoked.
        name: String,
        /// Truncated preview of the tool input.
        preview: String,
    },
    /// The preceding [`OutputEvent::ToolCall`] succeeded.
    ToolOk,
    /// The preceding [`OutputEvent::ToolCall`] failed, with the error text.
    ToolErr(String),
}

/// A [`UserOutput`] implementation that forwards events to a channel.
///
/// Sends are fire-and-forget: if the receiver has been dropped the event is
/// silently discarded rather than surfacing an error, so a closed TUI never
/// breaks the agent loop.
pub struct ChannelOutput(pub UnboundedSender<OutputEvent>);

impl UserOutput for ChannelOutput {
    fn info(&self, msg: &str) {
        let _ = self.0.send(OutputEvent::Info(msg.to_string()));
    }

    fn warn(&self, msg: &str) {
        let _ = self.0.send(OutputEvent::Warn(msg.to_string()));
    }

    fn error(&self, msg: &str) {
        let _ = self.0.send(OutputEvent::Error(msg.to_string()));
    }

    fn assistant_text(&self, text: &str) {
        let _ = self.0.send(OutputEvent::Chunk(text.to_string()));
    }

    fn tool_call(&self, tool_name: &str, input_preview: &str) {
        let _ = self.0.send(OutputEvent::ToolCall {
            name: tool_name.to_string(),
            preview: input_preview.to_string(),
        });
    }

    fn tool_ok(&self) {
        let _ = self.0.send(OutputEvent::ToolOk);
    }

    fn tool_err(&self, err_msg: &str) {
        let _ = self.0.send(OutputEvent::ToolErr(err_msg.to_string()));
    }
}

/// Applies a single output event to the in-progress turn's live text
/// buffer. Pure and unit-testable independent of the channel/terminal.
pub fn apply_output_event(live: &mut String, event: OutputEvent) {
    match event {
        OutputEvent::Chunk(text) => live.push_str(&text),
        OutputEvent::Info(msg) => live.push_str(&format!("\n[info] {msg}\n")),
        OutputEvent::Warn(msg) => live.push_str(&format!("\n[warn] {msg}\n")),
        OutputEvent::Error(msg) => live.push_str(&format!("\n[error] {msg}\n")),
        OutputEvent::ToolCall { name, preview } => {
            live.push_str(&format!("\n[tool: {name}] {preview}\n"));
        }
        OutputEvent::ToolOk => live.push_str(" ok\n"),
        OutputEvent::ToolErr(err) => live.push_str(&format!(" error: {err}\n")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_appends_verbatim() {
        let mut live = String::new();
        apply_output_event(&mut live, OutputEvent::Chunk("hello".into()));
        apply_output_event(&mut live, OutputEvent::Chunk(" world".into()));
        assert_eq!(live, "hello world");
    }

    #[test]
    fn tool_call_then_ok_reads_naturally() {
        let mut live = String::new();
        apply_output_event(
            &mut live,
            OutputEvent::ToolCall {
                name: "Read".into(),
                preview: "file.rs".into(),
            },
        );
        apply_output_event(&mut live, OutputEvent::ToolOk);
        assert_eq!(live, "\n[tool: Read] file.rs\n ok\n");
    }

    #[test]
    fn error_is_bracketed() {
        let mut live = String::new();
        apply_output_event(&mut live, OutputEvent::Error("boom".into()));
        assert_eq!(live, "\n[error] boom\n");
    }
}
