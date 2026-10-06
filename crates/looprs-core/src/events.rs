use std::collections::HashMap;

domain_event!(Event {
    SessionStart,
    SessionEnd,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    InferenceComplete,
    OnError,
    OnWarning,
    DelegationStart,
    DelegationComplete,
});

/// Context data that flows through events
#[derive(Debug, Clone)]
pub struct EventContext {
    /// Aggregated session context text available at event time.
    pub session_context: Option<String>,
    /// User message that triggered the event, when applicable.
    pub user_message: Option<String>,
    /// Tool name for tool-related events.
    pub tool_name: Option<String>,
    /// Tool output payload for post-tool events.
    pub tool_output: Option<String>,
    /// Error message for error events.
    pub error: Option<String>,
    /// Warning message for warning events.
    pub warning: Option<String>,
    /// Arbitrary key-value metadata for custom hook/event logic.
    pub metadata: HashMap<String, String>,
}

impl EventContext {
    /// Create an empty context with every field unset.
    ///
    /// Populate it with the `with_*` builder methods; which fields are
    /// meaningful depends on the [`Event`] being fired.
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

    /// Attach accumulated session context, overwriting any previous value.
    pub fn with_session_context(mut self, ctx: String) -> Self {
        self.session_context = Some(ctx);
        self
    }

    /// Attach the triggering user message, overwriting any previous value.
    pub fn with_user_message(mut self, msg: String) -> Self {
        self.user_message = Some(msg);
        self
    }

    /// Attach the name of the tool involved, overwriting any previous value.
    pub fn with_tool_name(mut self, name: String) -> Self {
        self.tool_name = Some(name);
        self
    }

    /// Attach captured tool output, overwriting any previous value.
    pub fn with_tool_output(mut self, output: String) -> Self {
        self.tool_output = Some(output);
        self
    }

    /// Attach an error description, overwriting any previous value.
    ///
    /// Note there is no corresponding `with_warning`; set
    /// [`EventContext::warning`] directly for [`Event::OnWarning`].
    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    /// Insert one metadata entry, replacing any existing value for `key`.
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

/// Handler for events
pub type EventHandler = Box<dyn Fn(Event, &EventContext) + Send + Sync>;

/// Manager for dispatching events
pub struct EventManager {
    handlers: HashMap<Event, Vec<EventHandler>>,
}

impl EventManager {
    /// Create a manager with no registered handlers.
    pub fn new() -> Self {
        EventManager {
            handlers: HashMap::new(),
        }
    }

    /// Register `handler` for `event`.
    ///
    /// Handlers accumulate: registering multiple handlers for the same event
    /// runs all of them, in registration order.
    pub fn on<F>(&mut self, event: Event, handler: F)
    where
        F: Fn(Event, &EventContext) + Send + Sync + 'static,
    {
        self.handlers
            .entry(event)
            .or_default()
            .push(Box::new(handler));
    }

    /// Invoke every handler registered for `event`, in registration order.
    ///
    /// Dispatch is synchronous and firing an event with no handlers is a
    /// no-op. Handlers cannot report failure, so a misbehaving handler will
    /// not interrupt the others; panics propagate to the caller.
    pub fn fire(&self, event: Event, context: &EventContext) {
        if let Some(handlers) = self.handlers.get(&event) {
            for handler in handlers {
                handler(event, context);
            }
        }
    }

    /// Remove all handlers registered for `event`.
    ///
    /// Handlers for other events are unaffected.
    pub fn clear(&mut self, event: Event) {
        self.handlers.remove(&event);
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
    fn event_names_from_macro() {
        assert_eq!(Event::SessionStart.name(), "SessionStart");
        assert_eq!(Event::SessionEnd.name(), "SessionEnd");
        assert_eq!(Event::PreToolUse.name(), "PreToolUse");
        assert_eq!(Event::OnError.name(), "OnError");
        assert_eq!(Event::DelegationStart.name(), "DelegationStart");
    }

    #[test]
    fn event_context_builder() {
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
    fn event_context_metadata() {
        let ctx = EventContext::new()
            .with_metadata("key1".to_string(), "value1".to_string())
            .with_metadata("key2".to_string(), "value2".to_string());

        assert_eq!(ctx.metadata.get("key1"), Some(&"value1".to_string()));
        assert_eq!(ctx.metadata.get("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn event_manager_fire() {
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
    fn event_manager_multiple_handlers() {
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
    fn event_manager_clear() {
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
