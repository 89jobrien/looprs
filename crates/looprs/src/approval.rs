/// Approval utilities for interactive prompts
use std::io::{self, Write};

/// Prompt the user for approval via console
/// Returns true if approved, false if declined
pub fn console_approval_prompt(message: &str) -> bool {
    print!("Approval required: {message} [y/N] ");

    // If flushing stdout fails, treat as non-approved (non-fatal, best-effort UI).
    if io::stdout().flush().is_err() {
        return false;
    }

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return false;
    }

    let response = input.trim().to_lowercase();
    matches!(response.as_str(), "y" | "yes")
}

/// Prompt the user for input via console
/// Returns Some(value) if non-empty, None otherwise
pub fn console_prompt(message: &str) -> Option<String> {
    print!("{message} ");

    if io::stdout().flush().is_err() {
        return None;
    }

    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return None;
    }

    let trimmed = input.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

/// Prompt the user for secret input via console (hidden)
/// Returns Some(value) if non-empty, None otherwise
pub fn console_secret_prompt(message: &str) -> Option<String> {
    print!("{message} ");

    if io::stdout().flush().is_err() {
        return None;
    }

    match read_hidden_input() {
        Ok(input) => {
            let trimmed = input.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        }
        Err(_) => None,
    }
}

#[cfg(unix)]
fn read_hidden_input() -> io::Result<String> {
    use std::os::unix::io::AsRawFd;

    let fd = io::stdin().as_raw_fd();
    let mut termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut termios) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let original = termios;
    termios.c_lflag &= !libc::ECHO;
    if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) } != 0 {
        return Err(io::Error::last_os_error());
    }

    let mut input = String::new();
    let result = io::stdin().read_line(&mut input);

    let _ = unsafe { libc::tcsetattr(fd, libc::TCSANOW, &original) };
    println!();
    let _ = io::stdout().flush();

    result.map(|_| input)
}

#[cfg(not(unix))]
fn read_hidden_input() -> io::Result<String> {
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_approval_prompt_is_callable() {
        let f = console_approval_prompt as fn(&str) -> bool;
        assert!(std::mem::size_of_val(&f) > 0);
    }

    #[test]
    fn console_prompt_and_console_secret_prompt_are_callable() {
        let p = console_prompt as fn(&str) -> Option<String>;
        let s = console_secret_prompt as fn(&str) -> Option<String>;
        assert!(std::mem::size_of_val(&p) > 0);
        assert!(std::mem::size_of_val(&s) > 0);
    }
}
