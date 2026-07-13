//! UiOutput adapter — `UserOutput` port backed by `looprs::ui`.
//!
//! Bridges the hexagonal `UserOutput` port to the rich terminal renderer in
//! `crate::ui` (colour, sanitization, machine-log JSON). Used as the default
//! output adapter wired into `Agent` for CLI and REPL sessions.

use looprs_core::ports::UserOutput;

use crate::ui;

/// Delegates all `UserOutput` calls to `crate::ui::*`.
pub struct UiOutput;

impl UserOutput for UiOutput {
    fn info(&self, msg: &str) {
        ui::info(msg);
    }

    fn warn(&self, msg: &str) {
        ui::warn(msg);
    }

    fn error(&self, msg: &str) {
        ui::error(msg);
    }

    fn assistant_text(&self, text: &str) {
        ui::assistant_text(text);
    }

    fn tool_call(&self, tool_name: &str, input_preview: &str) {
        ui::tool_call(tool_name, input_preview);
    }

    fn tool_ok(&self) {
        ui::tool_ok();
    }

    fn tool_err(&self, err_msg: &str) {
        ui::tool_err(err_msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // IDEA(L1): conformance test — proves UiOutput satisfies the UserOutput contract.
    // Mirrors the test in terminal_output.rs and null_output.rs.
    #[test]
    fn conformance() {
        looprs_core::ports::test_contracts::assert_user_output_contract(&UiOutput);
    }
}
