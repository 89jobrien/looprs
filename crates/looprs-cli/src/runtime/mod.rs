//! Groups shared runtime bootstrapping, event metadata, and single-turn execution helpers.

pub mod events;
pub mod facade;
pub mod session;

pub use facade::{bootstrap_runtime, provider_bootstrap_report};
