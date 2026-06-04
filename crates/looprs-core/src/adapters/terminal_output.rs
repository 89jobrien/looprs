//! TerminalOutput adapter — `UserOutput` port backed by stdout/stderr.
//!
//! This is a minimal, dependency-free adapter. The full looprs terminal
//! output (with colour, sanitization, and machine-log JSON) is implemented
//! in `looprs::ui`. Use this adapter in contexts where those extras are
//! not available (e.g. tests, embedded use).

use crate::ports::UserOutput;

/// Writes output directly to stdout/stderr with no formatting or sanitization.
pub struct TerminalOutput;

impl UserOutput for TerminalOutput {
    fn info(&self, msg: &str) {
        println!("{msg}");
    }

    fn warn(&self, msg: &str) {
        eprintln!("warning: {msg}");
    }

    fn error(&self, msg: &str) {
        eprintln!("error: {msg}");
    }

    fn assistant_text(&self, text: &str) {
        println!("{text}");
    }

    fn tool_call(&self, tool_name: &str, input_preview: &str) {
        println!("tool: {tool_name}({input_preview})");
    }

    fn tool_ok(&self) {
        println!("  ok");
    }

    fn tool_err(&self, err_msg: &str) {
        println!("  error: {err_msg}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conformance() {
        crate::ports::test_contracts::assert_user_output_contract(&TerminalOutput);
    }
}
