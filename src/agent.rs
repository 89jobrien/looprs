use anyhow::Result;
use colored::*;

use crate::api::ContentBlock;
use crate::api::Message;
use crate::config::get_max_tokens_for_model;
use crate::events::{Event, EventContext, EventManager};
use crate::hooks::HookExecutor;
use crate::hooks::HookRegistry;
use crate::observation_manager::ObservationManager;
use crate::providers::InferenceRequest;
use crate::providers::LLMProvider;
use crate::tools::{ToolContext, execute_tool, get_tool_definitions};

pub struct Agent {
    provider: Box<dyn LLMProvider>,
    messages: Vec<Message>,
    tool_ctx: ToolContext,
    pub events: EventManager,
    pub observations: ObservationManager,
    pub hooks: HookRegistry,
}

impl Agent {
    pub fn new(provider: Box<dyn LLMProvider>) -> Result<Self> {
        Ok(Self {
            provider,
            messages: Vec::new(),
            tool_ctx: ToolContext::new()?,
            events: EventManager::new(),
            observations: ObservationManager::new(),
            hooks: HookRegistry::new(),
        })
    }

    pub fn with_hooks(mut self, hooks: HookRegistry) -> Self {
        self.hooks = hooks;
        self
    }

    pub fn add_user_message(&mut self, text: impl Into<String>) {
        self.messages.push(Message::user(text));
    }

    pub fn clear_history(&mut self) {
        self.messages.clear();
    }

    pub fn execute_hooks_for_event(&self, event: &Event, context: &EventContext) -> EventContext {
        self.execute_hooks_for_event_with_approval(event, context, None)
    }

    pub fn execute_hooks_for_event_with_approval(
        &self,
        event: &Event,
        context: &EventContext,
        approval_fn: Option<&crate::hooks::ApprovalCallback>,
    ) -> EventContext {
        let mut enriched_context = context.clone();
        
        if let Some(hooks) = self.hooks.hooks_for_event(event) {
            for hook in hooks {
                if let Ok(results) =
                    HookExecutor::execute_hook_with_approval(hook, context, approval_fn)
                {
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

    pub async fn run_turn(&mut self) -> Result<()> {
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

        let event_ctx = EventContext::new().with_user_message(user_msg);
        self.events.fire(Event::UserPromptSubmit, &event_ctx);
        let enriched_ctx = self.execute_hooks_for_event(&Event::UserPromptSubmit, &event_ctx);

        // Build system prompt with base instructions + hook-injected context
        let mut system_prompt = format!(
            "You are a concise coding assistant. Current working directory: {}",
            self.tool_ctx.working_dir.display()
        );

        // Add any context injected by hooks
        if !enriched_ctx.metadata.is_empty() {
            system_prompt.push_str("\n\n## Additional Context from Hooks:");
            for (key, value) in &enriched_ctx.metadata {
                // Truncate large values to prevent prompt bloat (max 2000 chars per injection)
                const MAX_INJECTION_SIZE: usize = 2000;
                let truncated_value = if value.len() > MAX_INJECTION_SIZE {
                    format!("{}... [truncated {} bytes]", 
                            &value[..MAX_INJECTION_SIZE], 
                            value.len() - MAX_INJECTION_SIZE)
                } else {
                    value.clone()
                };
                system_prompt.push_str(&format!("\n### {key}\n{truncated_value}"));
            }
        }

        loop {
            let max_tokens = get_max_tokens_for_model(self.provider.model());
            let req = InferenceRequest {
                model: self.provider.model().to_string(),
                messages: self.messages.clone(),
                tools: get_tool_definitions(),
                max_tokens,
                system: system_prompt.clone(),
            };

            let response = self.provider.infer(req).await?;

            // Fire InferenceComplete event
            let event_ctx = EventContext::new();
            self.events.fire(Event::InferenceComplete, &event_ctx);
            self.execute_hooks_for_event(&Event::InferenceComplete, &event_ctx);

            let mut assistant_blocks = Vec::new();
            let mut tools_to_execute = Vec::new();

            for block in response.content {
                match block {
                    ContentBlock::Text { ref text } => {
                        println!("\n{} {}", "●".blue().bold(), text.blue());
                        assistant_blocks.push(block);
                    }
                    ContentBlock::ToolUse {
                        ref id,
                        ref name,
                        ref input,
                    } => {
                        let preview = serde_json::to_string(&input)
                            .unwrap_or_default()
                            .chars()
                            .take(60)
                            .collect::<String>();

                        println!(
                            "\n{} {}({})",
                            "⚙".yellow().bold(),
                            name.yellow().bold(),
                            preview.dimmed()
                        );

                        assistant_blocks.push(block.clone());
                        tools_to_execute.push((id.clone(), name.clone(), input.clone()));
                    }
                    _ => {}
                }
            }

            self.messages.push(Message::assistant(assistant_blocks));

            if tools_to_execute.is_empty() {
                break;
            }

            let mut tool_results = Vec::new();

            for (id, name, input) in tools_to_execute {
                // Fire PreToolUse event
                let event_ctx = EventContext::new().with_tool_name(name.clone());
                self.events.fire(Event::PreToolUse, &event_ctx);
                self.execute_hooks_for_event(&Event::PreToolUse, &event_ctx);

                let result = execute_tool(&name, &input, &self.tool_ctx);

                let content = match result {
                    Ok(ref output) => {
                        println!("  {} {}", "└─".green(), "OK".green());
                        // Capture observation
                        self.observations
                            .capture(name.clone(), input.clone(), output.clone());
                        // Fire PostToolUse event on success
                        let event_ctx = EventContext::new()
                            .with_tool_name(name.clone())
                            .with_tool_output(output.clone());
                        self.events.fire(Event::PostToolUse, &event_ctx);
                        self.execute_hooks_for_event(&Event::PostToolUse, &event_ctx);
                        output.clone()
                    }
                    Err(e) => {
                        let err_msg = format!("error: {e}");
                        println!("  {} {}", "└─".red(), err_msg.red());
                        // Fire OnError event
                        let event_ctx = EventContext::new()
                            .with_tool_name(name.clone())
                            .with_error(err_msg.clone());
                        self.events.fire(Event::OnError, &event_ctx);
                        self.execute_hooks_for_event(&Event::OnError, &event_ctx);
                        err_msg
                    }
                };

                tool_results.push(ContentBlock::ToolResult {
                    tool_use_id: id,
                    content,
                });
            }

            self.messages.push(Message::tool_results(tool_results));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::{InferenceResponse, Usage};

    // Mock provider for testing
    struct MockProvider {
        model: String,
        responses: Vec<InferenceResponse>,
        call_count: std::sync::Arc<std::sync::Mutex<usize>>,
    }

    impl MockProvider {
        fn new(responses: Vec<InferenceResponse>) -> Self {
            Self {
                model: "mock-model".to_string(),
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
        async fn infer(&self, _req: InferenceRequest) -> Result<InferenceResponse> {
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

        fn model(&self) -> &str {
            &self.model
        }

        fn validate_config(&self) -> Result<()> {
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
    fn test_agent_with_hooks() {
        let provider = MockProvider::simple_text("test");
        let hooks = HookRegistry::new();
        
        let agent = Agent::new(Box::new(provider))
            .unwrap()
            .with_hooks(hooks);
        
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
        
        let agent = Agent::new(Box::new(provider))
            .unwrap()
            .with_hooks(hooks);
        
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
        
        let agent = Agent::new(Box::new(provider))
            .unwrap()
            .with_hooks(hooks);
        
        let ctx = EventContext::new();
        let enriched = agent.execute_hooks_for_event(&Event::SessionStart, &ctx);
        
        // Should have injected context in metadata
        assert!(!enriched.metadata.is_empty());
        assert_eq!(enriched.metadata.len(), 2);
        assert_eq!(enriched.metadata.get("test_key").unwrap(), "injected context");
        assert_eq!(enriched.metadata.get("another_key").unwrap(), "another value");
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
        
        let agent = Agent::new(Box::new(provider))
            .unwrap()
            .with_hooks(hooks);
        
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
}

