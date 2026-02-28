//! Slot types for generative components
//!
//! This module defines the cache and status types for LLM-generated content slots.

use std::collections::HashMap;
use std::time::Instant;

/// Status of a generation request
#[derive(Debug, Clone, PartialEq)]
pub enum GenerationStatus {
    Pending,
    Generating,
    Cached { generated_at: Instant },
    Failed { error: String, fallback_active: bool },
}

/// Cache for generated slot values
#[derive(Debug, Clone, Default)]
pub struct SlotCache {
    text: HashMap<String, String>,
    colors: HashMap<String, (u8, u8, u8)>,
    styles: HashMap<String, GeneratedStyle>,
    status: HashMap<String, GenerationStatus>,
}

impl SlotCache {
    /// Create a new empty slot cache
    pub fn new() -> Self {
        Self {
            text: HashMap::new(),
            colors: HashMap::new(),
            styles: HashMap::new(),
            status: HashMap::new(),
        }
    }

    /// Get text from cache
    pub fn get_text(&self, key: &str) -> Option<&String> {
        self.text.get(key)
    }

    /// Set text in cache and mark as cached
    pub fn set_text(&mut self, key: String, value: String) {
        self.text.insert(key.clone(), value);
        self.status.insert(
            key,
            GenerationStatus::Cached {
                generated_at: Instant::now(),
            },
        );
    }

    /// Get color from cache
    pub fn get_color(&self, key: &str) -> Option<(u8, u8, u8)> {
        self.colors.get(key).copied()
    }

    /// Set color in cache and mark as cached
    pub fn set_color(&mut self, key: String, value: (u8, u8, u8)) {
        self.colors.insert(key.clone(), value);
        self.status.insert(
            key,
            GenerationStatus::Cached {
                generated_at: Instant::now(),
            },
        );
    }

    /// Get style from cache
    pub fn get_style(&self, key: &str) -> Option<&GeneratedStyle> {
        self.styles.get(key)
    }

    /// Set style in cache and mark as cached
    pub fn set_style(&mut self, slot_id: String, value: GeneratedStyle) {
        self.styles.insert(slot_id.clone(), value);
        self.status.insert(
            slot_id,
            GenerationStatus::Cached {
                generated_at: Instant::now(),
            },
        );
    }

    /// Get generation status for a key
    pub fn get_status(&self, key: &str) -> GenerationStatus {
        self.status
            .get(key)
            .cloned()
            .unwrap_or(GenerationStatus::Pending)
    }

    /// Mark a key as failed
    pub fn set_failed(&mut self, key: String, error: String, fallback_active: bool) {
        self.status.insert(
            key,
            GenerationStatus::Failed {
                error,
                fallback_active,
            },
        );
    }
}

/// Generated style properties
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedStyle {
    pub background: Option<(u8, u8, u8)>,
    pub corner_radius: Option<f32>,
    pub padding: Option<f32>,
    pub border_color: Option<(u8, u8, u8)>,
    pub border_width: Option<f32>,
}
