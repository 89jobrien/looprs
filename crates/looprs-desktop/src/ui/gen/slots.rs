//! Slot types for generative components
//!
//! This module defines the cache and status types for LLM-generated content slots.

use std::collections::HashMap;

/// Status of a generation request
#[derive(Debug, Clone, PartialEq)]
pub enum GenerationStatus {
    /// Not yet requested
    Pending,
    /// Currently being generated
    InProgress,
    /// Successfully generated
    Complete,
    /// Generation failed with error message
    Failed(String),
}

/// Cache entry for a generated value
#[derive(Debug, Clone)]
pub struct CacheEntry<T> {
    /// The generated value
    pub value: T,
    /// Status of the generation
    pub status: GenerationStatus,
}

impl<T> CacheEntry<T> {
    /// Create a new cache entry with pending status
    pub fn pending(value: T) -> Self {
        Self {
            value,
            status: GenerationStatus::Pending,
        }
    }

    /// Create a new cache entry with complete status
    pub fn complete(value: T) -> Self {
        Self {
            value,
            status: GenerationStatus::Complete,
        }
    }

    /// Create a new cache entry with failed status
    pub fn failed(value: T, error: String) -> Self {
        Self {
            value,
            status: GenerationStatus::Failed(error),
        }
    }

    /// Check if generation is complete
    pub fn is_complete(&self) -> bool {
        matches!(self.status, GenerationStatus::Complete)
    }

    /// Check if generation is in progress
    pub fn is_in_progress(&self) -> bool {
        matches!(self.status, GenerationStatus::InProgress)
    }

    /// Mark as in progress
    pub fn mark_in_progress(&mut self) {
        self.status = GenerationStatus::InProgress;
    }

    /// Mark as complete and update value
    pub fn mark_complete(&mut self, value: T) {
        self.value = value;
        self.status = GenerationStatus::Complete;
    }

    /// Mark as failed with error
    pub fn mark_failed(&mut self, error: String) {
        self.status = GenerationStatus::Failed(error);
    }
}

/// Cache for generated slot values
#[derive(Debug, Clone)]
pub struct SlotCache {
    /// Cached text values by slot key
    pub text: HashMap<String, CacheEntry<String>>,
    /// Cached color values by slot key (hex format)
    pub color: HashMap<String, CacheEntry<String>>,
    /// Cached style values by slot key
    pub style: HashMap<String, CacheEntry<GeneratedStyle>>,
}

impl Default for SlotCache {
    fn default() -> Self {
        Self::new()
    }
}

impl SlotCache {
    /// Create a new empty slot cache
    pub fn new() -> Self {
        Self {
            text: HashMap::new(),
            color: HashMap::new(),
            style: HashMap::new(),
        }
    }

    /// Clear all cached values
    pub fn clear(&mut self) {
        self.text.clear();
        self.color.clear();
        self.style.clear();
    }

    /// Get text from cache
    pub fn get_text(&self, key: &str) -> Option<&CacheEntry<String>> {
        self.text.get(key)
    }

    /// Get color from cache
    pub fn get_color(&self, key: &str) -> Option<&CacheEntry<String>> {
        self.color.get(key)
    }

    /// Get style from cache
    pub fn get_style(&self, key: &str) -> Option<&CacheEntry<GeneratedStyle>> {
        self.style.get(key)
    }
}

/// Generated style properties
#[derive(Debug, Clone, Default)]
pub struct GeneratedStyle {
    /// Font size
    pub font_size: Option<f32>,
    /// Font weight
    pub font_weight: Option<String>,
    /// Text color (hex format)
    pub color: Option<String>,
    /// Background color (hex format)
    pub background: Option<String>,
    /// Padding
    pub padding: Option<f32>,
    /// Margin
    pub margin: Option<f32>,
}
