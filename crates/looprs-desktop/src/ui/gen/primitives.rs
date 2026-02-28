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

impl Default for GenText {
    fn default() -> Self {
        Self {
            text: String::new(),
            font_size: 14.0,
            color: (0, 0, 0),
            slot_id: None,
            reactive_fields: Vec::new(),
        }
    }
}

impl GenText {
    pub fn new() -> Self {
        Self::default()
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

impl Component for GenText {
    fn render(&self) -> impl IntoElement {
        // Check for GenerativeContext
        let gen_ctx = use_try_consume::<super::context::GenerativeContext>();

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
            .text(final_text)
            .color(final_color)
            .font_size(self.font_size)
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
