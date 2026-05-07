// TODO(#9)
//! Mock terminal for integration testing.
//!
//! `MockTerminal` provides a functional stand-in for a real terminal UI,
//! recording output and supporting programmatic input injection.

use std::sync::{Arc, Mutex};

/// Recorded output line from the mock terminal.
#[derive(Debug, Clone)]
pub struct TerminalLine {
    pub text: String,
}

/// Mock terminal that captures output for assertion in tests.
#[derive(Debug, Clone)]
pub struct MockTerminal {
    output: Arc<Mutex<Vec<TerminalLine>>>,
    input_buf: Arc<Mutex<Vec<u8>>>,
    title: String,
}

impl MockTerminal {
    /// Create a new mock terminal with the given title.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            output: Arc::new(Mutex::new(Vec::new())),
            input_buf: Arc::new(Mutex::new(Vec::new())),
            title: title.into(),
        }
    }

    /// Write bytes to the terminal's input buffer (simulates user keystrokes).
    pub fn write_input(&self, bytes: &[u8]) {
        self.input_buf.lock().unwrap().extend_from_slice(bytes);
    }

    /// Emit a line of text as if the terminal process produced it.
    pub fn emit_output(&self, line: impl Into<String>) {
        self.output.lock().unwrap().push(TerminalLine { text: line.into() });
    }

    /// Clear all recorded output.
    pub fn clear(&self) {
        self.output.lock().unwrap().clear();
    }

    /// Return a snapshot of all output lines.
    pub fn output_lines(&self) -> Vec<TerminalLine> {
        self.output.lock().unwrap().clone()
    }

    /// Return the number of output lines recorded.
    pub fn line_count(&self) -> usize {
        self.output.lock().unwrap().len()
    }

    /// Return all output as a single joined string (newline-separated).
    pub fn output_text(&self) -> String {
        self.output
            .lock()
            .unwrap()
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Return the pending input bytes.
    pub fn pending_input(&self) -> Vec<u8> {
        self.input_buf.lock().unwrap().clone()
    }

    /// Return the terminal title.
    pub fn title(&self) -> &str {
        &self.title
    }
}

impl Default for MockTerminal {
    fn default() -> Self {
        Self::new("mock-terminal")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_terminal_records_output() {
        let term = MockTerminal::new("test");
        term.emit_output("hello");
        term.emit_output("world");
        assert_eq!(term.line_count(), 2);
        assert_eq!(term.output_text(), "hello\nworld");
    }

    #[test]
    fn mock_terminal_clear_resets_output() {
        let term = MockTerminal::new("test");
        term.emit_output("line");
        term.clear();
        assert_eq!(term.line_count(), 0);
    }

    #[test]
    fn mock_terminal_captures_input() {
        let term = MockTerminal::new("test");
        term.write_input(b"ls\r");
        assert_eq!(term.pending_input(), b"ls\r");
    }

    #[test]
    fn mock_terminal_default_title() {
        let term = MockTerminal::default();
        assert_eq!(term.title(), "mock-terminal");
    }

    #[test]
    fn mock_terminal_clone_shares_state() {
        let term = MockTerminal::new("shared");
        let clone = term.clone();
        term.emit_output("from original");
        assert_eq!(clone.line_count(), 1);
    }
}
