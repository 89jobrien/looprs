//! looprs-core — domain types, ports, adapters, and macros.
//!
//! This crate contains the portable domain layer: pure types, port traits,
//! and the adapters that depend only on this crate. Infrastructure adapters
//! requiring `looprs` crate internals (e.g. `PluginsAdapter`, `RetryProvider`)
//! remain in `looprs::adapters`.

#[macro_use]
pub mod macros;

pub mod adapters;
pub mod ai_types;
pub mod api;
pub mod events;
pub mod observation;
pub mod ports;
pub mod types;
