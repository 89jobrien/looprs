# Generative Slot Components Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement base UI components (GenText, GenContainer) with LLM-generated content slots that fall back to
explicit props when generative context is unavailable.

**Architecture:** Slot-aware component wrappers around Freya primitives. GenerativeContext provider manages cache and
triggers LLM generation. Components check context at render time - use generated values if available, fallback to
explicit props otherwise. Zero-cost when unused.

**Tech Stack:** Rust, Freya (UI), BAML (LLM client), Tokio (async), looprs-desktop-baml-client (existing)

---

## Phase 1: Core Primitives (Minimal Viable Implementation)

### Task 1: Create gen module structure

**Files:**

- Create: `crates/looprs-desktop/src/ui/gen/mod.rs`
- Create: `crates/looprs-desktop/src/ui/gen/slots.rs`
- Create: `crates/looprs-desktop/src/ui/gen/context.rs`
- Create: `crates/looprs-desktop/src/ui/gen/primitives.rs`
- Modify: `crates/looprs-desktop/src/ui/mod.rs`

**Step 1: Create gen module with placeholder exports**

Create `crates/looprs-desktop/src/ui/gen/mod.rs`:

```rust
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
```

**Step 2: Add gen module to ui/mod.rs**

Edit `crates/looprs-desktop/src/ui/mod.rs` - add after existing module declarations:

```rust
pub mod gen;
```

**Step 3: Create placeholder files**

Create `crates/looprs-desktop/src/ui/gen/slots.rs`:

```rust
//! Slot caching and generation status tracking

use std::collections::HashMap;
use std::time::Instant;

/// Status of content generation for a slot
#[derive(Debug, Clone, PartialEq)]
pub enum GenerationStatus {
    /// Not yet requested
    Pending,
    /// Currently generating via LLM
    Generating,
    /// Cached with timestamp
    Cached { generated_at: Instant },
    /// Failed with error message and fallback flag
    Failed { error: String, fallback_active: bool },
}

/// Generated styling properties
#[derive(Debug, Clone, PartialEq)]
pub struct GeneratedStyle {
    pub background: Option<(u8, u8, u8)>,
    pub corner_radius: Option<f32>,
    pub padding: Option<f32>,
    pub border_color: Option<(u8, u8, u8)>,
    pub border_width: Option<f32>,
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get_text(&self, slot_id: &str) -> Option<&String> {
        self.text.get(slot_id)
    }

    pub fn set_text(&mut self, slot_id: String, value: String) {
        self.text.insert(slot_id.clone(), value);
        self.status.insert(
            slot_id,
            GenerationStatus::Cached {
                generated_at: Instant::now(),
            },
        );
    }

    pub fn get_color(&self, slot_id: &str) -> Option<(u8, u8, u8)> {
        self.colors.get(slot_id).copied()
    }

    pub fn set_color(&mut self, slot_id: String, value: (u8, u8, u8)) {
        self.colors.insert(slot_id.clone(), value);
        self.status.insert(
            slot_id,
            GenerationStatus::Cached {
                generated_at: Instant::now(),
            },
        );
    }

    pub fn get_style(&self, slot_id: &str) -> Option<&GeneratedStyle> {
        self.styles.get(slot_id)
    }

    pub fn get_status(&self, slot_id: &str) -> GenerationStatus {
        self.status
            .get(slot_id)
            .cloned()
            .unwrap_or(GenerationStatus::Pending)
    }

    pub fn set_failed(&mut self, slot_id: String, error: String) {
        self.status.insert(
            slot_id,
            GenerationStatus::Failed {
                error,
                fallback_active: true,
            },
        );
    }
}
```

Create `crates/looprs-desktop/src/ui/gen/context.rs`:

```rust
//! GenerativeContext provider and management

use super::slots::SlotCache;
use std::sync::{Arc, RwLock};

/// Generative context providing cached LLM-generated values
#[derive(Clone)]
pub struct GenerativeContext {
    cache: Arc<RwLock<SlotCache>>,
}

impl GenerativeContext {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(SlotCache::new())),
        }
    }

    pub fn get_text(&self, slot_id: &str) -> Option<String> {
        self.cache
            .read()
            .ok()?
            .get_text(slot_id)
            .map(|s| s.clone())
    }

    pub fn set_text(&self, slot_id: impl Into<String>, value: impl Into<String>) {
        if let Ok(mut cache) = self.cache.write() {
            cache.set_text(slot_id.into(), value.into());
        }
    }

    pub fn get_color(&self, slot_id: &str) -> Option<(u8, u8, u8)> {
        self.cache.read().ok()?.get_color(slot_id)
    }

    pub fn set_color(&self, slot_id: impl Into<String>, value: (u8, u8, u8)) {
        if let Ok(mut cache) = self.cache.write() {
            cache.set_color(slot_id.into(), value);
        }
    }
}

impl Default for GenerativeContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Provider component for GenerativeContext
/// Will be implemented with Freya's context system in next task
pub struct GenerativeProvider;

impl GenerativeProvider {
    pub fn new() -> Self {
        Self
    }
}
```

Create `crates/looprs-desktop/src/ui/gen/primitives.rs`:

```rust
//! Primitive generative components (GenText, GenContainer)

// Placeholder - will implement in next tasks
```

**Step 4: Verify compilation**

Run: `cargo check --package looprs-desktop`

Expected: PASS (all modules compile, even if empty)

**Step 5: Commit module structure**

```bash
git add crates/looprs-desktop/src/ui/gen/ crates/looprs-desktop/src/ui/mod.rs
git commit -m "feat(gen): add generative components module structure

- Create gen/ module with slots, context, primitives
- Add SlotCache for text, color, style caching
- Add GenerativeContext with read/write access
- Add GenerationStatus enum for tracking state
- Placeholder for GenerativeProvider (Freya integration next)

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 2: Implement GenText primitive with tests

**Files:**

- Modify: `crates/looprs-desktop/src/ui/gen/primitives.rs`
- Create: `crates/looprs-desktop/src/ui/gen/primitives_test.rs`

**Step 1: Write failing test for GenText fallback behavior**

Edit `crates/looprs-desktop/src/ui/gen/primitives.rs`:

```rust
//! Primitive generative components (GenText, GenContainer)

use freya::prelude::*;

/// Text component with generative slot support
#[derive(Clone, Debug, PartialEq)]
pub struct GenText {
    // Fallback props
    text: String,
    font_size: f32,
    color: (u8, u8, u8),

    // Generative slot
    slot_id: Option<String>,
    reactive_fields: Vec<String>,
}

impl GenText {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            font_size: 14.0,
            color: (0, 0, 0),
            slot_id: None,
            reactive_fields: Vec::new(),
        }
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn color(mut self, color: (u8, u8, u8)) -> Self {
        self.color = color;
        self
    }

    pub fn slot_id(mut self, id: impl Into<String>) -> Self {
        self.slot_id = Some(id.into());
        self
    }

    pub fn reactive_on(mut self, fields: &[&str]) -> Self {
        self.reactive_fields = fields.iter().map(|s| s.to_string()).collect();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gentext_builder_pattern() {
        let component = GenText::new()
            .text("Hello")
            .font_size(16.0)
            .color((255, 0, 0))
            .slot_id("test_slot")
            .reactive_on(&["urgency", "sentiment"]);

        assert_eq!(component.text, "Hello");
        assert_eq!(component.font_size, 16.0);
        assert_eq!(component.color, (255, 0, 0));
        assert_eq!(component.slot_id, Some("test_slot".to_string()));
        assert_eq!(component.reactive_fields, vec!["urgency", "sentiment"]);
    }

    #[test]
    fn test_gentext_defaults() {
        let component = GenText::new();
        assert_eq!(component.text, "");
        assert_eq!(component.font_size, 14.0);
        assert_eq!(component.color, (0, 0, 0));
        assert_eq!(component.slot_id, None);
        assert!(component.reactive_fields.is_empty());
    }
}
```

**Step 2: Run tests to verify they pass**

Run: `cargo test --package looprs-desktop --lib gen::primitives::tests`

Expected: PASS (2 tests)

**Step 3: Implement GenText render method with context check**

Add to `crates/looprs-desktop/src/ui/gen/primitives.rs` after GenText impl:

```rust
impl Component for GenText {
    fn render(&self) -> impl IntoElement {
        // TODO: Check for GenerativeContext using Freya's use_context
        // For now, always use fallback props
        let final_text = &self.text;
        let final_color = self.color;

        label()
            .text(final_text)
            .color(final_color)
            .font_size(self.font_size)
    }
}
```

**Step 4: Verify compilation**

Run: `cargo check --package looprs-desktop`

Expected: PASS

**Step 5: Commit GenText primitive**

```bash
git add crates/looprs-desktop/src/ui/gen/primitives.rs
git commit -m "feat(gen): implement GenText primitive with builder pattern

- Add GenText struct with text, font_size, color props
- Add slot_id and reactive_fields for generative slots
- Implement builder pattern (new, text, font_size, color, slot_id, reactive_on)
- Implement Component trait with Freya label rendering
- Add unit tests for builder pattern and defaults
- Context integration TODO (next task)

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 3: Integrate Freya context system with GenerativeContext

**Files:**

- Modify: `crates/looprs-desktop/src/ui/gen/context.rs`
- Modify: `crates/looprs-desktop/src/ui/gen/primitives.rs`

**Step 1: Add Freya context provider implementation**

Edit `crates/looprs-desktop/src/ui/gen/context.rs` - replace GenerativeProvider impl:

```rust
/// Provider component for GenerativeContext
#[derive(Clone, Copy, PartialEq)]
pub struct GenerativeProvider {
    context: GenerativeContext,
}

impl GenerativeProvider {
    pub fn new(context: GenerativeContext) -> Self {
        Self { context }
    }

    pub fn child(self, child: impl IntoElement) -> impl IntoElement {
        use_context_provider(|| self.context.clone(), child)
    }
}
```

**Step 2: Update GenText to use Freya context**

Edit `crates/looprs-desktop/src/ui/gen/primitives.rs` - update render impl:

```rust
impl Component for GenText {
    fn render(&self) -> impl IntoElement {
        // Check for GenerativeContext
        let gen_ctx = use_context::<GenerativeContext>();

        // Get generated text if slot exists and context has it
        let final_text = if let (Some(slot_id), Some(ctx)) = (&self.slot_id, gen_ctx.as_ref()) {
            ctx.get_text(slot_id).unwrap_or_else(|| self.text.clone())
        } else {
            self.text.clone()
        };

        // Get generated color if available
        let final_color = if let (Some(slot_id), Some(ctx)) = (&self.slot_id, gen_ctx.as_ref()) {
            ctx.get_color(slot_id).unwrap_or(self.color)
        } else {
            self.color
        };

        label()
            .text(&final_text)
            .color(final_color)
            .font_size(self.font_size)
    }
}
```

**Step 3: Verify compilation**

Run: `cargo check --package looprs-desktop`

Expected: PASS

**Step 4: Commit context integration**

```bash
git add crates/looprs-desktop/src/ui/gen/context.rs crates/looprs-desktop/src/ui/gen/primitives.rs
git commit -m "feat(gen): integrate GenerativeContext with Freya context system

- Implement GenerativeProvider using use_context_provider
- Update GenText to check GenerativeContext via use_context
- Fallback to explicit props when context unavailable
- Context-aware rendering: generated text/color if available

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 4: Implement GenContainer primitive

**Files:**

- Modify: `crates/looprs-desktop/src/ui/gen/primitives.rs`

**Step 1: Add GenContainer struct with tests**

Add to `crates/looprs-desktop/src/ui/gen/primitives.rs` after GenText impl:

```rust
/// Container component with generative styling support
#[derive(Clone)]
pub struct GenContainer {
    // Layout props (not generative)
    width: Size,
    height: Size,
    direction: Direction,

    // Style props (fallbacks)
    background: (u8, u8, u8),
    corner_radius: f32,
    padding: f32,
    border_color: Option<(u8, u8, u8)>,
    border_width: f32,

    // Generative slot
    slot_id: Option<String>,
    reactive_fields: Vec<String>,

    // Children
    children: Vec<Element>,
}

impl GenContainer {
    pub fn new() -> Self {
        Self {
            width: Size::Auto,
            height: Size::Auto,
            direction: Direction::Vertical,
            background: (255, 255, 255),
            corner_radius: 0.0,
            padding: 0.0,
            border_color: None,
            border_width: 0.0,
            slot_id: None,
            reactive_fields: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn width(mut self, width: Size) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: Size) -> Self {
        self.height = height;
        self
    }

    pub fn vertical(mut self) -> Self {
        self.direction = Direction::Vertical;
        self
    }

    pub fn horizontal(mut self) -> Self {
        self.direction = Direction::Horizontal;
        self
    }

    pub fn background(mut self, color: (u8, u8, u8)) -> Self {
        self.background = color;
        self
    }

    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    pub fn border_color(mut self, color: (u8, u8, u8)) -> Self {
        self.border_color = Some(color);
        self
    }

    pub fn border_width(mut self, width: f32) -> Self {
        self.border_width = width;
        self
    }

    pub fn slot_id(mut self, id: impl Into<String>) -> Self {
        self.slot_id = Some(id.into());
        self
    }

    pub fn reactive_on(mut self, fields: &[&str]) -> Self {
        self.reactive_fields = fields.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn child(mut self, child: impl IntoElement) -> Self {
        self.children.push(child.into_element());
        self
    }

    pub fn children(mut self, children: impl IntoIterator<Item = Element>) -> Self {
        self.children.extend(children);
        self
    }
}

impl Default for GenContainer {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 2: Add GenContainer tests**

Add to test module in `crates/looprs-desktop/src/ui/gen/primitives.rs`:

```rust
#[test]
fn test_gencontainer_builder_pattern() {
    let component = GenContainer::new()
        .width(Size::Fill)
        .background((200, 200, 200))
        .corner_radius(8.0)
        .padding(16.0)
        .slot_id("test_container");

    assert_eq!(component.width, Size::Fill);
    assert_eq!(component.background, (200, 200, 200));
    assert_eq!(component.corner_radius, 8.0);
    assert_eq!(component.padding, 16.0);
    assert_eq!(component.slot_id, Some("test_container".to_string()));
}

#[test]
fn test_gencontainer_defaults() {
    let component = GenContainer::new();
    assert_eq!(component.background, (255, 255, 255));
    assert_eq!(component.corner_radius, 0.0);
    assert_eq!(component.padding, 0.0);
    assert_eq!(component.border_color, None);
}
```

**Step 3: Run tests**

Run: `cargo test --package looprs-desktop --lib gen::primitives::tests`

Expected: PASS (4 tests total)

**Step 4: Implement GenContainer render with context**

Add after GenContainer impl:

```rust
impl Component for GenContainer {
    fn render(&self) -> impl IntoElement {
        // Check for GenerativeContext
        let gen_ctx = use_context::<GenerativeContext>();

        // Get generated style if available
        let final_background = if let (Some(slot_id), Some(ctx)) = (&self.slot_id, gen_ctx.as_ref()) {
            ctx.get_color(slot_id).unwrap_or(self.background)
        } else {
            self.background
        };

        // Build base rect
        let mut container = rect()
            .width(self.width)
            .height(self.height)
            .background(final_background)
            .corner_radius(self.corner_radius)
            .padding(Gaps::new_all(self.padding));

        // Apply direction
        container = match self.direction {
            Direction::Vertical => container.vertical(),
            Direction::Horizontal => container.horizontal(),
        };

        // Add border if specified
        if let Some(border_color) = self.border_color {
            container = container.border(Border::new().fill(border_color).width(self.border_width));
        }

        // Add children
        for child in &self.children {
            container = container.child(child.clone());
        }

        container
    }
}
```

**Step 5: Verify compilation**

Run: `cargo check --package looprs-desktop`

Expected: PASS

**Step 6: Commit GenContainer primitive**

```bash
git add crates/looprs-desktop/src/ui/gen/primitives.rs
git commit -m "feat(gen): implement GenContainer primitive with styling

- Add GenContainer with layout (width, height, direction) and style props
- Support background, corner_radius, padding, border styling
- Implement context-aware rendering for background color
- Add builder pattern for all props including children
- Add unit tests for builder and defaults

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

### Task 5: Create integration demo showing context usage

**Files:**

- Create: `crates/looprs-desktop/src/ui/gen_demo.rs`
- Modify: `crates/looprs-desktop/src/ui/mod.rs`
- Modify: `crates/looprs-desktop/src/ui/root.rs`

**Step 1: Create demo screen**

Create `crates/looprs-desktop/src/ui/gen_demo.rs`:

```rust
//! Demo screen showing generative slot components

use crate::ui::gen::{GenContainer, GenText, GenerativeContext, GenerativeProvider};
use freya::prelude::*;

/// Gen components demo screen - entry point
pub fn gen_demo_screen() -> Element {
    Element::from(GenDemoScreen)
}

#[derive(Clone, Copy, PartialEq)]
pub struct GenDemoScreen;

impl Component for GenDemoScreen {
    fn render(&self) -> impl IntoElement {
        // Create generative context with some pre-loaded values
        let gen_ctx = use_memo(|| {
            let ctx = GenerativeContext::new();
            ctx.set_text("demo_title", "AI-Generated Title ✨");
            ctx.set_color("demo_title", (33, 150, 243)); // Blue
            ctx.set_text("demo_body", "This text was generated by the LLM!");
            ctx.set_color("demo_container", (200, 230, 201)); // Green 100
            ctx
        });

        GenerativeProvider::new(gen_ctx.read().clone()).child(
            rect()
                .width(Size::fill())
                .height(Size::fill())
                .vertical()
                .padding(Gaps::new_all(24.0))
                .background((18, 18, 18))
                .child(
                    // Section 1: GenText with slot (uses generated values)
                    GenText::new()
                        .text("Fallback Title")
                        .font_size(28.0)
                        .color((255, 255, 255))
                        .slot_id("demo_title"),
                )
                .child(
                    // Section 2: GenText without slot (uses fallback)
                    GenText::new()
                        .text("This uses fallback props (no slot)")
                        .font_size(14.0)
                        .color((150, 150, 150)),
                )
                .child(
                    // Section 3: GenContainer with slot
                    GenContainer::new()
                        .width(Size::fill())
                        .background((255, 255, 255))
                        .corner_radius(8.0)
                        .padding(16.0)
                        .slot_id("demo_container")
                        .child(
                            GenText::new()
                                .text("Fallback body text")
                                .font_size(14.0)
                                .color((0, 0, 0))
                                .slot_id("demo_body"),
                        ),
                )
                .child(
                    // Section 4: GenContainer without slot (fallback styling)
                    GenContainer::new()
                        .width(Size::fill())
                        .background((66, 66, 66))
                        .corner_radius(8.0)
                        .padding(16.0)
                        .child(
                            GenText::new()
                                .text("No slot - uses fallback gray background")
                                .font_size(14.0)
                                .color((255, 255, 255)),
                        ),
                ),
        )
    }
}
```

**Step 2: Add gen_demo to ui/mod.rs**

Edit `crates/looprs-desktop/src/ui/mod.rs`:

```rust
pub mod gen;
pub mod gen_demo;
```

**Step 3: Add demo to root navigation**

Edit `crates/looprs-desktop/src/ui/root.rs` - add to AppState enum and navigation logic (find existing
patterns and add gen_demo similarly).

**Step 4: Test the demo**

Run: `cargo run --package looprs-desktop`

Expected: Application launches, can navigate to gen demo, see generated vs fallback content

**Step 5: Commit demo**

```bash
git add crates/looprs-desktop/src/ui/gen_demo.rs crates/looprs-desktop/src/ui/mod.rs \
    crates/looprs-desktop/src/ui/root.rs
git commit -m "feat(gen): add demo screen showing generative slots

- Create gen_demo screen with GenerativeProvider setup
- Pre-load context with text/color values for demo
- Show GenText with slot (uses generated) vs without (fallback)
- Show GenContainer with slot (generated bg) vs without (fallback)
- Add to root navigation

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>"
```

---

## Phase 2: Error Handling (Next Steps)

### Task 6: Add GenerativeError types

**Files:**

- Create: `crates/looprs-desktop/src/ui/gen/error.rs`
- Modify: `crates/looprs-desktop/src/ui/gen/mod.rs`
- Modify: `crates/looprs-desktop/src/ui/gen/slots.rs`

**Overview:** Define error types (NetworkTimeout, RateLimitExceeded, InvalidSchema, etc.) and integrate with
GenerationStatus enum.

### Task 7: Implement retry policy

**Files:**

- Modify: `crates/looprs-desktop/src/ui/gen/context.rs`
- Create: `crates/looprs-desktop/src/ui/gen/retry.rs`

**Overview:** Exponential backoff (100ms, 500ms, 2s), rate limit handling, max 3 attempts for transient errors.

### Task 8: Add error callbacks to components

**Files:**

- Modify: `crates/looprs-desktop/src/ui/gen/primitives.rs`

**Overview:** Add `.on_generation_error()` method accepting closures for custom error handling.

---

## Phase 3: Reactivity (Future Tasks)

### Task 9: Implement reactive field tracking

**Files:**

- Modify: `crates/looprs-desktop/src/ui/gen/context.rs`
- Modify: `crates/looprs-desktop/src/ui/gen/slots.rs`

**Overview:** Track which slots watch which UiContext fields, invalidate cache when fields change.

### Task 10: Integrate with ContextEngine

**Files:**

- Modify: `crates/looprs-desktop/src/ui/gen/context.rs`
- Modify: `crates/looprs-desktop/src/services/context_engine.rs`

**Overview:** Subscribe to UiContext watch channel, trigger cache invalidation on updates.

### Task 11: Add manual regeneration API

**Files:**

- Modify: `crates/looprs-desktop/src/ui/gen/context.rs`

**Overview:** `regenerate(slot_id)` method, `regenerate_all()` for bulk updates.

---

## Phase 4: Composed Components (Future Tasks)

### Task 12: Implement GenButton

**Files:**

- Create: `crates/looprs-desktop/src/ui/gen/composed.rs`

**Overview:** Compose GenContainer + GenText, support `on_press` callbacks.

### Task 13: Implement GenCard

**Files:**

- Modify: `crates/looprs-desktop/src/ui/gen/composed.rs`

**Overview:** Card with title, body, actions - all using gen primitives.

### Task 14: Implement GenAlert and GenBadge

**Files:**

- Modify: `crates/looprs-desktop/src/ui/gen/composed.rs`

**Overview:** Alert banners and badge components with urgency-based styling.

---

## Phase 5: Observability & Testing (Future Tasks)

### Task 15: Add observability logging

**Files:**

- Modify: `crates/looprs-desktop/src/ui/gen/context.rs`

**Overview:** Log generation requests to `.looprs/observability/` with slot_id, latency, errors.

### Task 16: Add performance benchmarks

**Files:**

- Create: `crates/looprs-desktop/benches/gen_components.rs`

**Overview:** Benchmark cache hit render (<1ms target), invalidation (<100μs target).

### Task 17: Comprehensive integration tests

**Files:**

- Create: `crates/looprs-desktop/tests/gen_integration_test.rs`

**Overview:** Test with BAML client, verify generated text/colors, test error scenarios.

---

## Success Criteria

After Phase 1 (Tasks 1-5):

- [ ] GenText renders with fallback props when no context
- [ ] GenText uses generated text/color when context has values
- [ ] GenContainer renders with fallback styling when no context
- [ ] GenContainer uses generated background when context has values
- [ ] Demo screen shows all four scenarios (gen vs fallback for text and container)
- [ ] `cargo check`, `cargo test`, `cargo run` all succeed
- [ ] Zero compiler warnings

After All Phases:

- [ ] Performance: Cache hit render <1ms, invalidation <100μs
- [ ] Reliability: >95% generation success rate, <3s P95 latency
- [ ] Developer Experience: <10 lines to add generative slot
- [ ] User Experience: Smooth progressive enhancement, no loading flashes

---

## Testing Commands

```bash
# Run all tests
cargo test --package looprs-desktop

# Run only gen module tests
cargo test --package looprs-desktop --lib gen

# Run specific test
cargo test --package looprs-desktop --lib gen::primitives::tests::test_gentext_builder_pattern

# Check compilation
cargo check --package looprs-desktop

# Run application
cargo run --package looprs-desktop

# Format code
cargo fmt --package looprs-desktop

# Lint
cargo clippy --package looprs-desktop
```

---

## Implementation Notes

**TDD Approach:**

- Write test first (it should fail)
- Implement minimal code to pass
- Refactor if needed
- Commit after each passing test

**Freya Patterns:**

- Use `use_context` for reading context
- Use `use_context_provider` for providing context
- Use `use_memo` for expensive computations
- Components should be `Clone + PartialEq` for Freya's diffing

**BAML Integration:**

- Use existing `looprs_desktop_baml_client::B` for LLM calls
- Call `GenerateDynamicText` for text generation
- Handle async with tokio spawning
- Cache results to avoid repeated LLM calls

**Error Handling:**

- Never panic in render methods
- Always provide fallback values
- Log errors for observability
- Graceful degradation for user experience

---

## References

- Design Doc: `docs/plans/2026-02-28-generative-slot-components-design.md`
- Existing Context Engine: `crates/looprs-desktop/src/services/context_engine.rs`
- Existing Material Theme: `crates/looprs-desktop/src/ui/material/theme.rs`
- BAML Client: `crates/looprs-desktop-baml-client/src/baml_client/`
- Freya Docs: https://freyaui.dev/
