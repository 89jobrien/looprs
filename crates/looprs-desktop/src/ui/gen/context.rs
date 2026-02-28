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

impl GenerativeContext {
    /// Create a new generative context
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(SlotCache::new())),
        }
    }

    /// Get text from cache
    pub fn get_text(&self, key: &str) -> Option<String> {
        self.cache.read().unwrap().get_text(key).cloned()
    }

    /// Set text in cache
    pub fn set_text(&mut self, key: String, value: String) {
        self.cache.write().unwrap().set_text(key, value);
    }

    /// Get color from cache
    pub fn get_color(&self, key: &str) -> Option<(u8, u8, u8)> {
        self.cache.read().unwrap().get_color(key)
    }

    /// Set color in cache
    pub fn set_color(&mut self, key: String, value: (u8, u8, u8)) {
        self.cache.write().unwrap().set_color(key, value);
    }
}

impl Default for GenerativeContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Provider component for generative context
///
/// This will be implemented as a Freya component wrapper in the next task.
/// For now, it's a placeholder type.
pub struct GenerativeProvider;

impl GenerativeProvider {
    /// Create a new generative provider
    pub fn new() -> Self {
        Self
    }
}

impl Default for GenerativeProvider {
    fn default() -> Self {
        Self::new()
    }
}
