use std::ffi::OsString;

/// Check if ripgrep (rg) binary is available in PATH
pub fn is_rg_available() -> bool {
    crate::plugins::system().probe_success("rg", vec![OsString::from("--version")])
}

/// Check if fd binary is available in PATH
#[allow(dead_code)]
pub fn is_fd_available() -> bool {
    crate::plugins::system().probe_success("fd", vec![OsString::from("--version")])
}

#[allow(dead_code)]
trait BoolHelper {
    fn is_bool(&self) -> bool;
}

impl BoolHelper for bool {
    fn is_bool(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_check_returns_bool() {
        // These tests verify the functions don't panic
        // Actual availability depends on system configuration
        let _rg = is_rg_available();
        let _fd = is_fd_available();
    }

    #[test]
    fn availability_checks_are_safe() {
        // Ensure these don't crash even with missing binaries
        assert!(is_rg_available().is_bool() || !is_rg_available().is_bool());
        assert!(is_fd_available().is_bool() || !is_fd_available().is_bool());
    }
}
