//! Session logging — re-exported from `looprs-core`.
//!
//! The canonical `SessionEvent` and `SessionStore` trait live in
//! `looprs_core::ports::session_store`. The filesystem adapter is
//! `looprs_core::adapters::FsSessionStore`.
//!
//! This module re-exports them for backwards compatibility.

pub use looprs_core::adapters::FsSessionStore as SessionLogger;
pub use looprs_core::ports::session_store::SessionEvent;
pub use looprs_core::ports::session_store::SessionStore;
