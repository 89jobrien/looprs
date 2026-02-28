//! GenerativeContext for managing LLM-generated content
//!
//! This module provides the context and provider for generative components.

use super::slots::SlotCache;
use std::sync::{Arc, RwLock};

/// Context for generative components
///
/// Provides access to the slot cache and generation state.
/// Components read from this context to get LLM-generated values.
#[derive(Clone)]
pub struct GenerativeContext {
    /// Shared slot cache
    cache: Arc<RwLock<SlotCache>>,
}

impl PartialEq for GenerativeContext {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cache, &other.cache)
    }
}

impl GenerativeContext {
    /// Create a new generative context
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(SlotCache::new())),
        }
    }

    /// Get text from cache
    pub fn get_text(&self, key: &str) -> Option<String> {
        self.cache
            .read()
            .expect("SlotCache lock poisoned")
            .get_text(key)
            .cloned()
    }

    /// Set text in cache
    pub fn set_text(&self, key: String, value: String) {
        self.cache
            .write()
            .expect("SlotCache lock poisoned")
            .set_text(key, value);
    }

    /// Get color from cache
    pub fn get_color(&self, key: &str) -> Option<(u8, u8, u8)> {
        self.cache
            .read()
            .expect("SlotCache lock poisoned")
            .get_color(key)
    }

    /// Set color in cache
    pub fn set_color(&self, key: String, value: (u8, u8, u8)) {
        self.cache
            .write()
            .expect("SlotCache lock poisoned")
            .set_color(key, value);
    }

    /// Get style from cache
    pub fn get_style(&self, slot_id: &str) -> Option<super::slots::GeneratedStyle> {
        self.cache
            .read()
            .expect("SlotCache lock poisoned")
            .get_style(slot_id)
            .cloned()
    }

    /// Set style in cache
    pub fn set_style(&self, slot_id: String, value: super::slots::GeneratedStyle) {
        self.cache
            .write()
            .expect("SlotCache lock poisoned")
            .set_style(slot_id, value);
    }
}

impl Default for GenerativeContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Provider component for GenerativeContext
#[derive(Clone, PartialEq)]
pub struct GenerativeProvider {
    context: GenerativeContext,
}

impl GenerativeProvider {
    pub fn new(context: GenerativeContext) -> Self {
        Self { context }
    }

    pub fn child(self, child: impl freya::prelude::IntoElement) -> impl freya::prelude::IntoElement {
        freya::prelude::use_provide_context(|| self.context.clone());
        child
    }
}
