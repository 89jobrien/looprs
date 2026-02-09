/// Approval utilities for interactive prompts
use std::io::{self, Write};

/// Prompt the user for approval via console
/// Returns true if approved, false if declined
pub fn console_approval_prompt(message: &str) -> bool {
    print!("🔒 Approval required: {message} [y/N] ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    let response = input.trim().to_lowercase();
    matches!(response.as_str(), "y" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_console_prompt_exists() {
        // This test just verifies the function compiles and exists
        // Real testing would require mocking stdin
        let _fn = console_approval_prompt;
    }
}
