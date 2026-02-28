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

    /// Get read access to the cache
    pub fn with_cache<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&SlotCache) -> R,
    {
        let cache = self.cache.read().unwrap();
        f(&cache)
    }

    /// Get write access to the cache
    pub fn with_cache_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut SlotCache) -> R,
    {
        let mut cache = self.cache.write().unwrap();
        f(&mut cache)
    }

    /// Clear all cached values
    pub fn clear(&self) {
        self.with_cache_mut(|cache| cache.clear());
    }

    /// Get text from cache
    pub fn get_text(&self, key: &str) -> Option<String> {
        self.with_cache(|cache| cache.get_text(key).map(|entry| entry.value.clone()))
    }

    /// Get color from cache
    pub fn get_color(&self, key: &str) -> Option<String> {
        self.with_cache(|cache| cache.get_color(key).map(|entry| entry.value.clone()))
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
