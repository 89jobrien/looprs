# Generative Slot Components Design

**Date:** 2026-02-28
**Status:** Approved
**Pattern:** Slot-based generative UI (one of multiple patterns)

## Overview

Design for base UI components (GenText, GenContainer) with reserved fields for LLM-generated content (text, colors, styling). Components use generative values when context is available, fall back to explicit props otherwise. Complex components inherit generative capabilities through composition.

**Core Principle:** AI integration at the primitive layer eliminates dependency threading in composed components.

## Architecture

### Component Hierarchy

```
Primitives (GenText, GenContainer)
    ↓ compose into
Base Components (GenButton, GenCard, GenAlert, GenBadge)
    ↓ compose into
Complex Components (StatusPanel, ErrorDashboard, MetricsGrid)
```

### Generative Slots

- Each Gen* component has optional `slot_id` and `reactive_fields`
- Components check `GenerativeContext` (Freya context system) at render time
- If context exists and has cached value for slot_id → use generated value
- If no context or no cached value → use explicit fallback props
- No slot machinery in Freya primitives themselves (wrappers only)

### File Structure

```
crates/looprs-desktop/src/ui/gen/
├── mod.rs              # Exports Gen* components
├── primitives.rs       # GenText, GenContainer
├── context.rs          # GenerativeContext provider
├── slots.rs            # Slot registration, caching, updates
└── composed.rs         # GenButton, GenCard, etc.
```

### Zero-Cost When Unused

If you never use Gen* components or provide GenerativeContext, zero runtime cost. Regular Freya components work unchanged.

## Component Design

### GenText - Primitive Text Component

```rust
pub struct GenText {
    // Explicit props (fallbacks)
    text: String,
    font_size: f32,
    color: (u8, u8, u8),
    weight: FontWeight,

    // Generative slots (optional)
    slot_id: Option<String>,
    reactive_fields: Vec<String>,  // e.g., ["urgency", "sentiment"]
}

impl GenText {
    pub fn new() -> Self { /* defaults */ }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
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

impl Component for GenText {
    fn render(&self) -> impl IntoElement {
        // Check for GenerativeContext
        let gen_ctx = use_context::<GenerativeContext>();

        // Get generated value if slot exists and context has it
        let final_text = if let (Some(slot_id), Some(ctx)) = (&self.slot_id, gen_ctx) {
            ctx.get_text(slot_id).unwrap_or(&self.text)
        } else {
            &self.text
        };

        let final_color = if let (Some(slot_id), Some(ctx)) = (&self.slot_id, gen_ctx) {
            ctx.get_color(slot_id).unwrap_or(self.color)
        } else {
            self.color
        };

        // Render using Freya primitives
        label()
            .text(final_text)
            .color(final_color)
            .font_size(self.font_size)
    }
}
```

### GenContainer - Primitive Container

```rust
pub struct GenContainer {
    // Layout props (not generative)
    width: Size,
    height: Size,
    direction: Direction,  // Vertical | Horizontal

    // Style props (fallbacks)
    background: (u8, u8, u8),
    corner_radius: f32,
    padding: f32,
    border_color: Option<(u8, u8, u8)>,
    border_width: f32,

    // Generative slots
    slot_id: Option<String>,
    reactive_fields: Vec<String>,

    // Children
    children: Vec<Element>,
}
```

Similar render logic: check context, use generated colors/borders/corners if available, fallback to explicit props.

### Composed Component Example - GenButton

```rust
pub fn GenButton(
    label: &str,
    on_press: impl Fn() + 'static,
    slot_id: Option<&str>,
) -> Element {
    GenContainer::new()
        .slot_id(slot_id.map(|s| format!("{}_container", s)))
        .background((100, 100, 100))  // Fallback
        .corner_radius(8.0)
        .padding(12.0)
        .on_press(on_press)
        .child(
            GenText::new()
                .text(label)
                .slot_id(slot_id.map(|s| format!("{}_label", s)))
                .color((255, 255, 255))
        )
}
```

GenButton composes GenContainer + GenText. Both primitives check context independently. No special button logic needed - inheritance through composition.

## Data Flow & Context Management

### GenerativeContext Structure

```rust
pub struct GenerativeContext {
    // Cached generated values indexed by slot_id
    text_cache: HashMap<String, String>,
    color_cache: HashMap<String, (u8, u8, u8)>,
    style_cache: HashMap<String, GeneratedStyle>,

    // Current context signals (drives generation)
    ui_context: Arc<UiContext>,  // From context_engine

    // Reactive tracking
    watchers: HashMap<String, Vec<String>>,  // slot_id -> reactive_fields
    generation_status: HashMap<String, GenerationStatus>,
}

pub enum GenerationStatus {
    Pending,
    Generating,
    Cached { generated_at: Instant },
    Failed { error: String, fallback_active: bool },
}
```

### Context Provider Pattern

```rust
// In root component
GenerativeProvider::new()
    .with_context_engine(context_engine_handle)
    .child(app_content)

// Hybrid scoping - override in subtrees
GenerativeProvider::new()
    .with_urgency_override(UrgencyLevel::Critical)
    .child(error_panel)  // This panel always uses critical styling
```

**Hybrid Scoping (D):** Provider sets default context, individual components can override. Enables region-specific moods/tones in multi-region dashboards (status panel uses calm context, error panel uses urgent context within same app).

### Update Flow

**Hybrid Update Strategy (E):**

1. **Default: Lazy generation** - Generate once, cache until explicitly invalidated
2. **Opt-in reactive:** Developer marks fields as reactive (e.g., urgency changes trigger regeneration)
3. **Manual control:** Explicit `regenerate()` for user-initiated refreshes

**Detailed Flow:**

1. **Context Engine** (existing) produces UiContext updates via watch channel
2. **GenerativeProvider** subscribes to UiContext updates
3. On update, provider checks which slots have reactive fields matching changed context
4. For reactive slots: invalidate cache, trigger async regeneration
5. For lazy slots: keep cached value until manual `regenerate()` call
6. Components re-render automatically when cache updates (Freya reactivity)

### Generation Request Flow

```
Component renders with slot_id="status_msg"
    ↓
Checks GenerativeContext.text_cache["status_msg"]
    ↓
Cache miss → Request generation
    ↓
GenerativeProvider calls BAML client (GenerateDynamicText)
    ↓
Async response → Update cache → Component re-renders
```

### Latency Handling

- First render: Show fallback immediately (no flash of loading state)
- Generation happens async in background
- Cache update triggers re-render with generated content
- Progressive enhancement: fallback → generated

## Multiple Generative Patterns

**Slot-based (this design) is ONE pattern among many:**

```rust
// Pattern 1: Slot-based (GenText, GenContainer)
GenText::new()
    .text("Status: OK")
    .slot_id("status_msg")

// Pattern 2: Full tree generation (existing)
let ui_tree = B.GenerateSentimentAwareUi
    .call(goal, state, sentiment)
    .await?;
render_baml_tree(ui_tree);

// Pattern 3: Streaming updates (future)
StreamingCard::new()
    .prompt("Summarize system health")
    .on_chunk(|chunk| /* update UI */)

// Pattern 4: Declarative DSL (future)
GenTemplate::parse("Show {metric} with {severity} styling")
    .bind("metric", cpu_usage)
    .bind("severity", urgency)
```

**Why multiple patterns matter:**

- **Slot-based:** Predictable structure, fine-grained control, fast updates
- **Full tree gen:** Flexible layouts, creative compositions, novel UI structures
- **Streaming:** Real-time LLM output, progressive disclosure
- **DSL:** Developer-friendly, template reuse

All patterns can share the same GenerativeContext and context engine. Components choose pattern based on use case.

## Error Handling

### Error Categories

```rust
pub enum GenerativeError {
    // Transient - retry possible
    NetworkTimeout { slot_id: String, attempt: u32 },
    RateLimitExceeded { retry_after: Duration },

    // Critical - permanent failure
    InvalidSchema { slot_id: String, error: String },
    ContextUnavailable,

    // Degraded - partial failure
    PartialGeneration { generated: HashMap<String, String>, failed: Vec<String> },
}
```

### Error Handling Strategy (B + C + E)

**Transient failures** (network timeout, rate limit):
```rust
// Visual indicator on component
GenText::new()
    .text("Status: OK")  // Fallback shown
    .slot_id("status")
    // Renders with subtle dimmed indicator or icon
    // Auto-retry in background (3 attempts with backoff)
```

**Critical failures** (schema error, permanent API issue):
```rust
// Error state replacement
if let GenerationStatus::Failed { error, .. } = status {
    rect()
        .background((255, 235, 235))  // Light red
        .child(label().text(format!("⚠️ Generation failed: {}", error)))
}
```

**Developer callbacks (E):**
```rust
GenText::new()
    .text("Status: OK")
    .slot_id("status")
    .on_generation_error(|error| {
        match error {
            GenerativeError::NetworkTimeout { .. } => {
                // Custom logging, metrics, retry logic
                metrics::increment_counter("gen_timeout");
            }
            GenerativeError::InvalidSchema { .. } => {
                // Alert developer, this is a bug
                panic!("Schema error in status slot");
            }
            _ => {}
        }
    })
```

### Retry Policy

- Transient errors: 3 retries with exponential backoff (100ms, 500ms, 2s)
- Rate limits: Honor retry_after header
- Critical errors: No retry, immediate fallback
- Developer can override via `.retry_policy()`

### Observability

- All generation attempts logged to `.looprs/observability/`
- Includes: slot_id, context snapshot, error type, retry count, latency
- Metrics: success rate, P95 latency, cache hit rate per slot

## Testing Strategy

### Unit Tests - Component Behavior

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gentext_fallback_when_no_context() {
        let component = GenText::new()
            .text("Fallback")
            .slot_id("test_slot");

        // Render without GenerativeContext
        let rendered = render_without_context(component);
        assert_eq!(rendered.text(), "Fallback");
    }

    #[test]
    fn test_gentext_uses_generated_when_available() {
        let mut ctx = GenerativeContext::new();
        ctx.set_text("test_slot", "Generated!");

        let component = GenText::new()
            .text("Fallback")
            .slot_id("test_slot");

        let rendered = render_with_context(component, ctx);
        assert_eq!(rendered.text(), "Generated!");
    }

    #[test]
    fn test_reactive_invalidation() {
        let mut ctx = GenerativeContext::new();
        ctx.set_text("status", "Healthy");

        // Update urgency (reactive field)
        ctx.update_urgency(5);

        // Cache should be invalidated
        assert_eq!(ctx.get_status("status"), GenerationStatus::Pending);
    }
}
```

### Integration Tests - BAML Generation

```rust
#[tokio::test]
async fn test_dynamic_text_generation() {
    let ctx = UiContext {
        sentiment: Some(SentimentContext {
            sentiment: Sentiment::Negative,
            severity: Severity::Critical,
            mood: Mood::Urgent,
            ..Default::default()
        }),
        ..Default::default()
    };

    let template = "{{severity_prefix}} {{message_count}} messages";
    let mut vars = HashMap::new();
    vars.insert("message_count".into(), "5".into());

    let result = B.GenerateDynamicText
        .call(template, vars, ctx.sentiment.sentiment, ctx.sentiment.mood)
        .await
        .unwrap();

    // Should include urgency indicators for critical/urgent
    assert!(result.contains("Critical") || result.contains("⚠️"));
}
```

### Visual Regression Tests - Freya Testing

```rust
#[tokio::test]
#[serial]  // Freya tests must run serially
async fn test_genbutton_critical_styling() {
    let mut ctx = GenerativeContext::new();
    ctx.set_urgency(5);  // Critical

    let app = GenerativeProvider::new()
        .with_context(ctx)
        .child(GenButton::new("Alert", || {}, Some("test_btn")));

    let mut utils = launch_test(app);
    utils.wait_for_update().await;

    // Should have red background for critical urgency
    let button = utils.get_by_text("Alert");
    assert_eq!(button.background_color(), (244, 67, 54));  // Red 500
}
```

### Error Handling Tests

```rust
#[tokio::test]
async fn test_network_timeout_fallback() {
    let mock_baml = MockBamlClient::new()
        .with_timeout_for("status_slot");

    let component = GenText::new()
        .text("Fallback text")
        .slot_id("status_slot");

    let rendered = render_with_mock_client(component, mock_baml);

    // Should show fallback immediately
    assert_eq!(rendered.text(), "Fallback text");
    // Should have visual indicator
    assert!(rendered.has_class("gen-degraded"));
}
```

### Performance Tests

```rust
#[bench]
fn bench_gentext_render_with_cache_hit(b: &mut Bencher) {
    let ctx = GenerativeContext::with_preloaded_cache();
    b.iter(|| {
        render_with_context(GenText::new().slot_id("test"), ctx.clone())
    });
    // Target: <1ms for cache hit
}

#[bench]
fn bench_context_update_propagation(b: &mut Bencher) {
    let ctx = GenerativeContext::new();
    b.iter(|| {
        ctx.update_urgency(3);
        ctx.invalidate_reactive_slots();
    });
    // Target: <100μs for invalidation
}
```

## Design Decisions Summary

| Decision Point | Choice | Rationale |
|---------------|--------|-----------|
| **Approach** | Slot-Aware Component Wrappers (A) | Clean separation, explicit AI usage, Freya compatibility |
| **Primitives** | GenText + GenContainer | Minimal API surface, composition for complex components |
| **Slot Control** | Full control available (C) | Text, color, font size, weight, borders, corners, spacing |
| **Opt-in Pattern** | Context-aware (D) | Single prop definition, automatic adaptation |
| **Update Strategy** | Hybrid (E) | Lazy default, reactive opt-in, manual triggers |
| **Error Handling** | Visual indicator + Error state + Callbacks (B+C+E) | Transient vs critical failures, developer override |
| **Context Scoping** | Hybrid (D) | Provider default, component override for multi-region UIs |

## Implementation Phases

### Phase 1: Core Primitives
- GenerativeContext structure and provider
- GenText primitive with slot support
- GenContainer primitive with slot support
- Basic cache and fallback logic

### Phase 2: Error Handling
- Error types and categorization
- Retry policy implementation
- Visual indicators and error states
- Developer callback hooks

### Phase 3: Reactivity
- Reactive field tracking
- Cache invalidation on context changes
- Manual regeneration API
- Performance optimization

### Phase 4: Composed Components
- GenButton, GenCard, GenAlert
- GenBadge, GenChip
- Complex dashboard components

### Phase 5: Observability & Testing
- Logging integration
- Metrics collection
- Comprehensive test suite
- Performance benchmarks

## Success Metrics

- **Performance:** Cache hit render <1ms, invalidation <100μs
- **Reliability:** >95% generation success rate, <3s P95 latency
- **Developer Experience:** <10 lines of code to add generative slot to component
- **User Experience:** Smooth progressive enhancement, no loading flashes
