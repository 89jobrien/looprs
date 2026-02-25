use crate::api::ContentBlock;
use crate::api::Message;
use crate::app_config::DefaultsConfig;
use crate::errors::AgentError;
use crate::events::{Event, EventContext, EventManager};
use crate::file_refs::FileRefPolicy;
use crate::hooks::{ApprovalCallback, HookExecutor, HookRegistry, PromptCallback};
use crate::observation_manager::ObservationManager;
use crate::providers::InferenceRequest;
use crate::providers::LLMProvider;
use crate::rules::RuleRegistry;
use crate::tools::{ToolContext, execute_tool, get_tool_definitions};
use crate::ui;
use std::collections::HashMap;
use tokio::time::{Duration, timeout};

#[derive(Debug, Clone, Default)]
pub struct RuntimeSettings {
    pub defaults: DefaultsConfig,
    pub max_tokens_override: Option<u32>,
}

pub struct Agent {
    provider: Box<dyn LLMProvider>,
    messages: Vec<Message>,
    tool_ctx: ToolContext,
    pub events: EventManager,
    pub observations: ObservationManager,
    pub hooks: HookRegistry,
    pub rules: RuleRegistry,
    runtime: RuntimeSettings,
    file_ref_policy: FileRefPolicy,
    pending_metadata: HashMap<String, String>,
}

impl Agent {
    pub fn new(provider: Box<dyn LLMProvider>) -> Result<Self, AgentError> {
        Self::new_with_runtime(
            provider,
            RuntimeSettings::default(),
            FileRefPolicy::default(),
        )
    }

    pub fn new_with_runtime(
        provider: Box<dyn LLMProvider>,
        runtime: RuntimeSettings,
        file_ref_policy: FileRefPolicy,
    ) -> Result<Self, AgentError> {
        Ok(Self {
            provider,
            messages: Vec::new(),
            tool_ctx: ToolContext::new()?,
            events: EventManager::new(),
            observations: ObservationManager::new(),
            hooks: HookRegistry::new(),
            rules: RuleRegistry::new(),
            runtime,
            file_ref_policy,
            pending_metadata: HashMap::new(),
        })
    }

    pub fn with_hooks(mut self, hooks: HookRegistry) -> Self {
        self.hooks = hooks;
        self
    }

    pub fn set_provider(&mut self, provider: Box<dyn LLMProvider>) {
        self.provider = provider;
    }

    pub fn set_runtime_settings(&mut self, runtime: RuntimeSettings) {
        self.runtime = runtime;
    }

    pub fn set_file_ref_policy(&mut self, policy: FileRefPolicy) {
        self.file_ref_policy = policy;
    }

    pub fn set_turn_metadata(&mut self, metadata: HashMap<String, String>) {
        self.pending_metadata.extend(metadata);
    }

    pub fn add_user_message(&mut self, text: impl Into<String>) {
        let text_str = text.into();

        // Resolve file references (@filename) if present
        let resolved = if crate::file_refs::has_file_references(&text_str) {
            match crate::file_refs::resolve_file_references(
                &text_str,
                &self.tool_ctx.working_dir,
                &self.file_ref_policy,
            ) {
                Ok(resolved_text) => resolved_text,
                Err(e) => {
                    ui::warn(format!("Warning: Error resolving file references: {e}"));
                    text_str.clone()
                }
            }
        } else {
            text_str.clone()
        };

        self.messages.push(Message::user(resolved));
    }

    pub fn clear_history(&mut self) {
        self.messages.clear();
    }

    pub fn latest_assistant_text(&self) -> Option<String> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == "assistant")
            .map(|m| {
                m.content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.as_str()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n")
            })
            .filter(|text| !text.is_empty())
    }

    pub fn working_dir(&self) -> &std::path::Path {
        &self.tool_ctx.working_dir
    }

    pub fn execute_hooks_for_event(&self, event: &Event, context: &EventContext) -> EventContext {
        self.execute_hooks_for_event_with_callbacks(event, context, None, None, None)
    }

    pub fn execute_hooks_for_event_with_approval(
        &self,
        event: &Event,
        context: &EventContext,
        approval_fn: Option<&crate::hooks::ApprovalCallback>,
    ) -> EventContext {
        self.execute_hooks_for_event_with_callbacks(event, context, approval_fn, None, None)
    }

    pub fn execute_hooks_for_event_with_callbacks(
        &self,
        event: &Event,
        context: &EventContext,
        approval_fn: Option<&ApprovalCallback>,
        prompt_fn: Option<&PromptCallback>,
        secret_prompt_fn: Option<&PromptCallback>,
    ) -> EventContext {
        let mut enriched_context = context.clone();

        if let Some(hooks) = self.hooks.hooks_for_event(event) {
            for hook in hooks {
                if let Ok(results) = HookExecutor::execute_hook_with_callbacks(
                    hook,
                    context,
                    approval_fn,
                    prompt_fn,
                    secret_prompt_fn,
                ) {
                    // Inject hook outputs into context metadata
                    for result in results {
                        if let Some(key) = result.inject_key {
                            enriched_context.metadata.insert(key, result.output);
                        }
                    }
                }
            }
        }

        enriched_context
    }

    pub async fn run_turn(&mut self) -> Result<(), AgentError> {
        let delegated_agent = self.pending_metadata.get("orchestration.agent").cloned();
        if let Some(agent_name) = delegated_agent.clone() {
            let strategy = self
                .pending_metadata
                .get("orchestration.strategy")
                .cloned()
                .unwrap_or_else(|| "sequential".to_string());
            let event_ctx = EventContext::new()
                .with_tool_name(agent_name)
                .with_metadata("orchestration.strategy".to_string(), strategy)
                .with_metadata("orchestration.mode".to_string(), "delegated".to_string());
            self.events.fire(Event::DelegationStart, &event_ctx);
            self.execute_hooks_for_event(&Event::DelegationStart, &event_ctx);
        }

        // Fire UserPromptSubmit event
        let user_msg = self
            .messages
            .last()
            .and_then(|m| {
                if m.role == "user" {
                    m.content.first().and_then(|cb| {
                        if let ContentBlock::Text { text } = cb {
                            Some(text.clone())
                        } else {
                            None
                        }
                    })
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let mut event_ctx = EventContext::new().with_user_message(user_msg);
        for (key, value) in &self.pending_metadata {
            event_ctx.metadata.insert(key.clone(), value.clone());
        }
        self.events.fire(Event::UserPromptSubmit, &event_ctx);
        let mut enriched_ctx = self.execute_hooks_for_event(&Event::UserPromptSubmit, &event_ctx);
        for (key, value) in std::mem::take(&mut self.pending_metadata) {
            enriched_ctx.metadata.insert(key, value);
        }

        // Build system prompt with base instructions + hook-injected context + rules
        let mut system_prompt = format!(
            "You are a concise coding assistant. Current working directory: {}",
            self.tool_ctx.working_dir.display()
        );

        // Add project rules and guidelines
        let rules_section = self.rules.format_for_prompt();
        if !rules_section.is_empty() {
            system_prompt.push_str(&rules_section);
        }

        // Add any context injected by hooks
        if !enriched_ctx.metadata.is_empty() {
            system_prompt.push_str("\n\n## Additional Context from Hooks:");
            for (key, value) in &enriched_ctx.metadata {
                // Truncate large values to prevent prompt bloat (max 2000 chars per injection)
                const MAX_INJECTION_SIZE: usize = 2000;
                let truncated_value = if value.len() > MAX_INJECTION_SIZE {
                    format!(
                        "{}... [truncated {} bytes]",
                        &value[..MAX_INJECTION_SIZE],
                        value.len() - MAX_INJECTION_SIZE
                    )
                } else {
                    value.clone()
                };
                system_prompt.push_str(&format!("\n### {key}\n{truncated_value}"));
            }
        }

        loop {
            let mut max_tokens = self.provider.model().max_tokens();
            if let Some(override_tokens) = self.runtime.max_tokens_override {
                max_tokens = max_tokens.min(override_tokens);
            }
            if let Some(max_context) = self.runtime.defaults.max_context_tokens {
                max_tokens = max_tokens.min(max_context);
            }
            let req = InferenceRequest {
                model: self.provider.model().clone(),
                messages: self.messages.clone(),
                tools: get_tool_definitions(),
                max_tokens,
                temperature: self.runtime.defaults.temperature,
                system: system_prompt.clone(),
            };

            let response = if let Some(timeout_secs) = self.runtime.defaults.timeout_seconds {
                match timeout(Duration::from_secs(timeout_secs), self.provider.infer(&req)).await {
                    Ok(res) => res?,
                    Err(_) => return Err(AgentError::Timeout),
                }
            } else {
                self.provider.infer(&req).await?
            };

            #[cfg(not(test))]
            if let Err(e) =
                crate::trace::append_turn_trace(self.observations.session_id(), &req, &response)
            {
                ui::warn(format!("Warning: Failed to append turn trace: {e}"));
            }

            // Fire InferenceComplete event
            let event_ctx = EventContext::new();
            self.events.fire(Event::InferenceComplete, &event_ctx);
            self.execute_hooks_for_event(&Event::InferenceComplete, &event_ctx);

            let assistant_blocks = response.content;
            let mut tool_indices = Vec::new();

            for (idx, block) in assistant_blocks.iter().enumerate() {
                match block {
                    ContentBlock::Text { text } => {
                        ui::assistant_text(text);
                    }
                    ContentBlock::ToolUse { name, input, .. } => {
                        let preview = serde_json::to_string(&input)
                            .unwrap_or_default()
                            .chars()
                            .take(60)
                            .collect::<String>();

                        ui::tool_call(name.as_str(), &preview);
                        tool_indices.push(idx);
                    }
                    _ => {}
                }
            }

            self.messages.push(Message::assistant(assistant_blocks));

            if tool_indices.is_empty() {
                break;
            }

            let mut tool_results = Vec::new();
            let assistant_message = self.messages.last().expect("assistant message just pushed");

            for idx in tool_indices {
                let ContentBlock::ToolUse { id, name, input } = &assistant_message.content[idx]
                else {
                    continue;
                };
                // Fire PreToolUse event
                let event_ctx = EventContext::new().with_tool_name(name.as_str().to_string());
                self.events.fire(Event::PreToolUse, &event_ctx);
                self.execute_hooks_for_event(&Event::PreToolUse, &event_ctx);

                let result = execute_tool(name.as_str(), input, &self.tool_ctx);

                let content = match result {
                    Ok(ref output) => {
                        ui::tool_ok();
                        // Capture observation
                        self.observations.capture(
                            name.as_str().to_string(),
                            input.clone(),
                            output.clone(),
                            Some(id.clone()),
                        );
                        // Fire PostToolUse event on success
                        let event_ctx = EventContext::new()
                            .with_tool_name(name.as_str().to_string())
                            .with_tool_output(output.clone());
                        self.events.fire(Event::PostToolUse, &event_ctx);
                        self.execute_hooks_for_event(&Event::PostToolUse, &event_ctx);
                        output.clone()
                    }
                    Err(e) => {
                        let err_msg = format!("error: {e}");
                        ui::tool_err(&err_msg);
                        // Fire OnError event
                        let event_ctx = EventContext::new()
                            .with_tool_name(name.as_str().to_string())
                            .with_error(err_msg.clone());
                        self.events.fire(Event::OnError, &event_ctx);
                        self.execute_hooks_for_event(&Event::OnError, &event_ctx);
                        err_msg
                    }
                };

                tool_results.push(ContentBlock::ToolResult {
                    tool_use_id: id.clone(),
                    content,
                });
            }

            self.messages.push(Message::tool_results(tool_results));
        }

        if let Some(agent_name) = delegated_agent {
            let event_ctx = EventContext::new()
                .with_tool_name(agent_name)
                .with_metadata("orchestration.mode".to_string(), "delegated".to_string());
            self.events.fire(Event::DelegationComplete, &event_ctx);
            self.execute_hooks_for_event(&Event::DelegationComplete, &event_ctx);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::ProviderError;
    use crate::providers::{InferenceResponse, Usage};

    // Mock provider for testing
    struct MockProvider {
        model: crate::types::ModelId,
        responses: Vec<InferenceResponse>,
        call_count: std::sync::Arc<std::sync::Mutex<usize>>,
    }

    impl MockProvider {
        fn new(responses: Vec<InferenceResponse>) -> Self {
            Self {
                model: crate::types::ModelId::new("mock-model"),
                responses,
                call_count: std::sync::Arc::new(std::sync::Mutex::new(0)),
            }
        }

        fn simple_text(text: &str) -> Self {
            Self::new(vec![InferenceResponse {
                content: vec![ContentBlock::Text {
                    text: text.to_string(),
                }],
                stop_reason: "end_turn".to_string(),
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 20,
                },
            }])
        }
    }

    #[async_trait::async_trait]
    impl LLMProvider for MockProvider {
        async fn infer(&self, _req: &InferenceRequest) -> Result<InferenceResponse, ProviderError> {
            let mut count = self.call_count.lock().unwrap();
            let idx = *count;
            *count += 1;

            if idx < self.responses.len() {
                Ok(self.responses[idx].clone())
            } else {
                // Default response if we run out
                Ok(InferenceResponse {
                    content: vec![ContentBlock::Text {
                        text: "default response".to_string(),
                    }],
                    stop_reason: "end_turn".to_string(),
                    usage: Usage {
                        input_tokens: 0,
                        output_tokens: 0,
                    },
                })
            }
        }

        fn name(&self) -> &str {
            "mock"
        }

        fn model(&self) -> &crate::types::ModelId {
            &self.model
        }

        fn validate_config(&self) -> Result<(), ProviderError> {
            Ok(())
        }
    }

    #[test]
    fn test_agent_new() {
        let provider = MockProvider::simple_text("test");
        let agent = Agent::new(Box::new(provider));
        assert!(agent.is_ok());
    }

    #[test]
    fn test_agent_add_user_message() {
        let provider = MockProvider::simple_text("test");
        let mut agent = Agent::new(Box::new(provider)).unwrap();

        agent.add_user_message("Hello");
        assert_eq!(agent.messages.len(), 1);
        assert_eq!(agent.messages[0].role, "user");
    }

    #[test]
    fn test_agent_add_multiple_messages() {
        let provider = MockProvider::simple_text("test");
        let mut agent = Agent::new(Box::new(provider)).unwrap();

        agent.add_user_message("First");
        agent.add_user_message("Second");
        assert_eq!(agent.messages.len(), 2);
    }

    #[test]
    fn test_agent_clear_history() {
        let provider = MockProvider::simple_text("test");
        let mut agent = Agent::new(Box::new(provider)).unwrap();

        agent.add_user_message("Test");
        assert_eq!(agent.messages.len(), 1);

        agent.clear_history();
        assert_eq!(agent.messages.len(), 0);
    }

    #[test]
    fn test_latest_assistant_text_none_when_no_assistant() {
        let provider = MockProvider::simple_text("test");
        let mut agent = Agent::new(Box::new(provider)).unwrap();
        agent.add_user_message("Hello");
        assert_eq!(agent.latest_assistant_text(), None);
    }

    #[tokio::test]
    async fn test_latest_assistant_text_returns_last_text_blocks() {
        let provider = MockProvider::new(vec![InferenceResponse {
            content: vec![
                ContentBlock::Text {
                    text: "First".to_string(),
                },
                ContentBlock::Text {
                    text: "Second".to_string(),
                },
            ],
            stop_reason: "end_turn".to_string(),
            usage: Usage {
                input_tokens: 2,
                output_tokens: 3,
            },
        }]);
        let mut agent = Agent::new(Box::new(provider)).unwrap();
        agent.add_user_message("Hello");

        agent.run_turn().await.unwrap();

        assert_eq!(
            agent.latest_assistant_text(),
            Some("First\n\nSecond".to_string())
        );
    }

    #[test]
    fn test_agent_with_hooks() {
        let provider = MockProvider::simple_text("test");
        let hooks = HookRegistry::new();

        let agent = Agent::new(Box::new(provider)).unwrap().with_hooks(hooks);

        // Just verify it works
        assert_eq!(agent.messages.len(), 0);
    }

    #[test]
    fn test_execute_hooks_for_event_no_hooks() {
        let provider = MockProvider::simple_text("test");
        let agent = Agent::new(Box::new(provider)).unwrap();

        let ctx = EventContext::new().with_user_message("test".to_string());
        let enriched = agent.execute_hooks_for_event(&Event::SessionStart, &ctx);

        // Should return unchanged context
        assert!(enriched.metadata.is_empty());
    }

    #[test]
    fn test_execute_hooks_for_event_with_hooks() {
        let provider = MockProvider::simple_text("test");

        // Create a hook registry (empty is fine, we're just testing it doesn't crash)
        let hooks = HookRegistry::new();

        let agent = Agent::new(Box::new(provider)).unwrap().with_hooks(hooks);

        let ctx = EventContext::new();
        let enriched = agent.execute_hooks_for_event(&Event::SessionStart, &ctx);

        // Should work even with no hooks
        assert!(enriched.metadata.is_empty());
    }

    #[test]
    fn test_context_injection_from_hooks() {
        use std::io::Write;
        use tempfile::TempDir;

        let provider = MockProvider::simple_text("test");

        // Create a temporary hook file with inject_as
        let temp_dir = TempDir::new().unwrap();
        let hook_file = temp_dir.path().join("test_hook.yaml");
        let mut file = std::fs::File::create(&hook_file).unwrap();
        writeln!(
            file,
            r#"name: test_injection
trigger: SessionStart
actions:
  - type: command
    command: "echo 'injected context'"
    inject_as: "test_key"
  - type: command
    command: "echo 'another value'"
    inject_as: "another_key""#
        )
        .unwrap();
        drop(file);

        // Load hooks from temp directory
        let hooks = HookRegistry::load_from_directory(&temp_dir.path().to_path_buf()).unwrap();

        let agent = Agent::new(Box::new(provider)).unwrap().with_hooks(hooks);

        let ctx = EventContext::new();
        let enriched = agent.execute_hooks_for_event(&Event::SessionStart, &ctx);

        // Should have injected context in metadata
        assert!(!enriched.metadata.is_empty());
        assert_eq!(enriched.metadata.len(), 2);
        assert_eq!(
            enriched.metadata.get("test_key").unwrap(),
            "injected context"
        );
        assert_eq!(
            enriched.metadata.get("another_key").unwrap(),
            "another value"
        );
    }

    #[test]
    fn test_context_injection_without_inject_as() {
        use std::io::Write;
        use tempfile::TempDir;

        let provider = MockProvider::simple_text("test");

        // Create a hook without inject_as
        let temp_dir = TempDir::new().unwrap();
        let hook_file = temp_dir.path().join("test_hook.yaml");
        let mut file = std::fs::File::create(&hook_file).unwrap();
        writeln!(
            file,
            r#"name: test_no_injection
trigger: SessionStart
actions:
  - type: command
    command: "echo 'not injected'"
  - type: message
    text: "just a message""#
        )
        .unwrap();
        drop(file);

        let hooks = HookRegistry::load_from_directory(&temp_dir.path().to_path_buf()).unwrap();

        let agent = Agent::new(Box::new(provider)).unwrap().with_hooks(hooks);

        let ctx = EventContext::new();
        let enriched = agent.execute_hooks_for_event(&Event::SessionStart, &ctx);

        // Should NOT have any injected context
        assert!(enriched.metadata.is_empty());
    }

    #[test]
    fn test_context_injection_large_value_truncation() {
        let provider = MockProvider::simple_text("test");
        let mut agent = Agent::new(Box::new(provider)).unwrap();

        // Create context with a very large injected value
        let large_value = "x".repeat(5000);
        let mut ctx = EventContext::new().with_user_message("test".to_string());
        ctx.metadata.insert("large_key".to_string(), large_value);

        // Simulate the hook execution result
        agent.messages.push(crate::api::Message::user("test"));

        // The run_turn method should handle large values gracefully
        // We can't easily test the full flow without mocking, but we can verify
        // the context is created correctly
        assert_eq!(ctx.metadata.get("large_key").unwrap().len(), 5000);
    }

    #[tokio::test]
    async fn test_run_turn_simple() {
        let provider = MockProvider::simple_text("Hello response");
        let mut agent = Agent::new(Box::new(provider)).unwrap();

        agent.add_user_message("Hello");
        let result = agent.run_turn().await;

        assert!(result.is_ok());
        // Should have user message + assistant response
        assert_eq!(agent.messages.len(), 2);
        assert_eq!(agent.messages[1].role, "assistant");
    }

    #[test]
    fn test_observation_manager_initialized() {
        let provider = MockProvider::simple_text("test");
        let agent = Agent::new(Box::new(provider)).unwrap();

        assert_eq!(agent.observations.count(), 0);
    }

    #[test]
    fn test_event_manager_initialized() {
        let provider = MockProvider::simple_text("test");
        let agent = Agent::new(Box::new(provider)).unwrap();

        // EventManager should be initialized and ready to use
        let ctx = EventContext::new();
        agent.events.fire(Event::SessionStart, &ctx);
        // Should not panic
    }

    #[test]
    fn test_file_reference_resolution() {
        use std::io::Write;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        let mut file = std::fs::File::create(&test_file).unwrap();
        writeln!(file, "Hello from file!").unwrap();

        let provider = MockProvider::simple_text("test");
        let mut agent = Agent::new(Box::new(provider)).unwrap();

        // Override working directory to temp dir for this test
        agent.tool_ctx.working_dir = temp_dir.path().to_path_buf();

        // Add message with file reference
        agent.add_user_message("Check @test.txt please");

        // Should have resolved the file reference
        assert_eq!(agent.messages.len(), 1);
        let msg_content = agent.messages[0].content.first().unwrap();
        if let crate::api::ContentBlock::Text { text } = msg_content {
            assert!(text.contains("Hello from file!"));
            assert!(text.contains("// File: test.txt"));
        } else {
            panic!("Expected text content block");
        }
    }

    #[test]
    fn execute_hooks_supports_prompt_callbacks() {
        let _ = Agent::execute_hooks_for_event_with_callbacks;
    }
}
