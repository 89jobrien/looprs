//! Shared runtime bootstrap, turn execution, and metadata helpers for CLI front ends.

pub mod events;
pub mod facade;
pub mod session;

pub use facade::{bootstrap_runtime, provider_bootstrap_report};
