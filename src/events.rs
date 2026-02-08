use std::collections::HashMap;

/// Represents lifecycle and execution events in looprs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Event {
    /// Session started - context available
    SessionStart,
    /// Session ending - cleanup time
    SessionEnd,
    /// User prompt submitted - before processing
    UserPromptSubmit,
    /// Tool use about to happen - approval gate
    PreToolUse,
    /// Tool use completed - success
    PostToolUse,
    /// LLM inference completed
    InferenceComplete,
    /// Error occurred during operation
    OnError,
    /// Warning issued (non-fatal issue)
    OnWarning,
}

impl Event {
    /// Get human-readable name for event
    pub fn name(&self) -> &'static str {
        match self {
            Event::SessionStart => "SessionStart",
            Event::SessionEnd => "SessionEnd",
            Event::UserPromptSubmit => "UserPromptSubmit",
            Event::PreToolUse => "PreToolUse",
            Event::PostToolUse => "PostToolUse",
            Event::InferenceComplete => "InferenceComplete",
            Event::OnError => "OnError",
            Event::OnWarning => "OnWarning",
        }
    }
}

/// Context data that flows through events
#[derive(Debug, Clone)]
pub struct EventContext {
    /// Session context from startup
    pub session_context: Option<String>,
    /// User message being processed
    pub user_message: Option<String>,
    /// Tool being executed
    pub tool_name: Option<String>,
    /// Tool result/output
    pub tool_output: Option<String>,
    /// Error message if applicable
    pub error: Option<String>,
    /// Warning message if applicable
    pub warning: Option<String>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl EventContext {
    /// Create a new empty context
    pub fn new() -> Self {
        EventContext {
            session_context: None,
            user_message: None,
            tool_name: None,
            tool_output: None,
            error: None,
            warning: None,
            metadata: HashMap::new(),
        }
    }

    /// Set session context
    pub fn with_session_context(mut self, ctx: String) -> Self {
        self.session_context = Some(ctx);
        self
    }

    /// Set user message
    pub fn with_user_message(mut self, msg: String) -> Self {
        self.user_message = Some(msg);
        self
    }

    /// Set tool name
    pub fn with_tool_name(mut self, name: String) -> Self {
        self.tool_name = Some(name);
        self
    }

    /// Set tool output
    pub fn with_tool_output(mut self, output: String) -> Self {
        self.tool_output = Some(output);
        self
    }

    /// Set error
    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    /// Set warning
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warning = Some(warning);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

impl Default for EventContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Handler for events - will be called when events fire
pub type EventHandler = Box<dyn Fn(Event, &EventContext) + Send + Sync>;

/// Manager for dispatching events
pub struct EventManager {
    handlers: HashMap<Event, Vec<EventHandler>>,
}

impl EventManager {
    /// Create a new event manager
    pub fn new() -> Self {
        EventManager {
            handlers: HashMap::new(),
        }
    }

    /// Register a handler for an event
    pub fn on<F>(&mut self, event: Event, handler: F)
    where
        F: Fn(Event, &EventContext) + Send + Sync + 'static,
    {
        self.handlers
            .entry(event)
            .or_default()
            .push(Box::new(handler));
    }

    /// Fire an event and call all registered handlers
    pub fn fire(&self, event: Event, context: &EventContext) {
        if let Some(handlers) = self.handlers.get(&event) {
            for handler in handlers {
                handler(event, context);
            }
        }
    }

    /// Clear all handlers for an event
    pub fn clear(&mut self, event: Event) {
        self.handlers.remove(&event);
    }

    /// Clear all handlers
    pub fn clear_all(&mut self) {
        self.handlers.clear();
    }
}

impl Default for EventManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn test_event_names() {
        assert_eq!(Event::SessionStart.name(), "SessionStart");
        assert_eq!(Event::SessionEnd.name(), "SessionEnd");
        assert_eq!(Event::PreToolUse.name(), "PreToolUse");
        assert_eq!(Event::OnError.name(), "OnError");
    }

    #[test]
    fn test_event_context_builder() {
        let ctx = EventContext::new()
            .with_session_context("session_data".to_string())
            .with_user_message("test message".to_string())
            .with_error("test error".to_string());

        assert_eq!(ctx.session_context, Some("session_data".to_string()));
        assert_eq!(ctx.user_message, Some("test message".to_string()));
        assert_eq!(ctx.error, Some("test error".to_string()));
        assert!(ctx.tool_name.is_none());
    }

    #[test]
    fn test_event_context_metadata() {
        let ctx = EventContext::new()
            .with_metadata("key1".to_string(), "value1".to_string())
            .with_metadata("key2".to_string(), "value2".to_string());

        assert_eq!(ctx.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(ctx.metadata.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_event_manager_fire() {
        let mut manager = EventManager::new();
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();

        manager.on(Event::SessionStart, move |_event, _ctx| {
            let mut c = counter_clone.lock().unwrap();
            *c += 1;
        });

        let ctx = EventContext::new();
        manager.fire(Event::SessionStart, &ctx);

        assert_eq!(*counter.lock().unwrap(), 1);
    }

    #[test]
    fn test_event_manager_multiple_handlers() {
        let mut manager = EventManager::new();
        let counter1 = Arc::new(Mutex::new(0));
        let counter1_clone = counter1.clone();
        let counter2 = Arc::new(Mutex::new(0));
        let counter2_clone = counter2.clone();

        manager.on(Event::PreToolUse, move |_event, _ctx| {
            let mut c = counter1_clone.lock().unwrap();
            *c += 1;
        });

        manager.on(Event::PreToolUse, move |_event, _ctx| {
            let mut c = counter2_clone.lock().unwrap();
            *c += 1;
        });

        let ctx = EventContext::new();
        manager.fire(Event::PreToolUse, &ctx);

        assert_eq!(*counter1.lock().unwrap(), 1);
        assert_eq!(*counter2.lock().unwrap(), 1);
    }

    #[test]
    fn test_event_manager_clear() {
        let mut manager = EventManager::new();
        let counter = Arc::new(Mutex::new(0));
        let counter_clone = counter.clone();

        manager.on(Event::SessionStart, move |_event, _ctx| {
            let mut c = counter_clone.lock().unwrap();
            *c += 1;
        });

        manager.clear(Event::SessionStart);
        let ctx = EventContext::new();
        manager.fire(Event::SessionStart, &ctx);

        assert_eq!(*counter.lock().unwrap(), 0);
    }
}
