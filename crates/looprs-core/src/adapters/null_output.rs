//! NullOutput adapter — a no-op `UserOutput` for tests and embedded use.

use crate::ports::UserOutput;

/// Discards all output. Use in tests where UI side-effects are unwanted.
pub struct NullOutput;

impl UserOutput for NullOutput {
    fn info(&self, _msg: &str) {}
    fn warn(&self, _msg: &str) {}
    fn error(&self, _msg: &str) {}
    fn assistant_text(&self, _text: &str) {}
    fn tool_call(&self, _tool_name: &str, _input_preview: &str) {}
    fn tool_ok(&self) {}
    fn tool_err(&self, _err_msg: &str) {}
}
