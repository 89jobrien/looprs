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

/// Container component with generative styling support
#[derive(Clone, PartialEq)]
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
            width: Size::auto(),
            height: Size::auto(),
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

impl Component for GenContainer {
    fn render(&self) -> impl IntoElement {
        // Check for GenerativeContext
        let gen_ctx = use_try_consume::<super::context::GenerativeContext>();

        // Get generated style if available
        let final_background = if let (Some(slot_id), Some(ctx)) = (&self.slot_id, gen_ctx.as_ref()) {
            ctx.get_color(slot_id).unwrap_or(self.background)
        } else {
            self.background
        };

        // Build base rect
        let mut container = rect()
            .width(self.width.clone())
            .height(self.height.clone())
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
}
