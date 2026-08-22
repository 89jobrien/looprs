//! A `UserOutput` port implementation that forwards every event over a
//! channel instead of writing to stdout, so the chat TUI can render
//! streamed assistant text inside its own ratatui frame in real time.

use looprs::ports::UserOutput;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub enum OutputEvent {
    Chunk(String),
    Info(String),
    Warn(String),
    Error(String),
    ToolCall { name: String, preview: String },
    ToolOk,
    ToolErr(String),
}

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
