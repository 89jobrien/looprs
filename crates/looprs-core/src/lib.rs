//! looprs-core — domain types, ports, and macros.
//!
//! This crate contains pure domain logic with zero infrastructure
//! dependencies. All external system interaction is defined via port
//! traits; adapters live in `looprs-adapters`.

#[macro_use]
pub mod macros;

pub mod api;
pub mod events;
pub mod observation;
pub mod ports;
pub mod types;
