//! looprs-core — domain types, ports, adapters, and macros.
//!
//! This crate contains the portable domain layer: pure types, port traits,
//! and the adapters that depend only on this crate. Infrastructure adapters
//! requiring `looprs` crate internals (e.g. `PluginsAdapter`, `RetryProvider`)
//! remain in `looprs::adapters`.

/// Reusable macros for typed IDs and domain events.
#[macro_use]
pub mod macros;

/// Infrastructure adapters that implement core ports.
pub mod adapters;
/// Shared AI-oriented domain types used by observability and analysis features.
pub mod ai_types;
/// API model types shared across crates.
pub mod api;
/// Event types and event context shared across runtime boundaries.
pub mod events;
/// Tool-observation schema used for persistence and replay.
pub mod observation;
/// Hexagonal port traits for runtime integrations.
pub mod ports;
/// Core newtype identifiers and model-related helpers.
pub mod types;
