use std::ffi::OsString;

use crate::plugins::NamedTool;
use crate::plugins::binaries::{Fd, Rg};

/// Check if ripgrep (rg) binary is available in PATH
pub fn is_rg_available() -> bool {
    Rg::system().probe_success(vec![OsString::from("--version")])
}

/// Check if fd binary is available in PATH
#[allow(dead_code)]
pub fn is_fd_available() -> bool {
    Fd::system().probe_success(vec![OsString::from("--version")])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_checks_do_not_panic() {
        let _rg = is_rg_available();
        let _fd = is_fd_available();
    }
}
