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
        let mut enriched_context = context.clone();
        
        if let Some(hooks) = self.hooks.hooks_for_event(event) {
            for hook in hooks {
                if let Ok(results) = HookExecutor::execute_hook(hook, context) {
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
                system_prompt.push_str(&format!("\n### {}\n{}", key, value));
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
