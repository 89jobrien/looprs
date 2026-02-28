//! Generative slot components - AI-powered UI primitives
//!
//! This module provides components with LLM-generated content slots.
//! Components use generated values when GenerativeContext is available,
//! fall back to explicit props otherwise.

pub mod context;
pub mod primitives;
pub mod slots;

pub use context::{GenerativeContext, GenerativeProvider};
pub use primitives::{GenContainer, GenText};
pub use slots::{GeneratedStyle, GenerationStatus, SlotCache};
