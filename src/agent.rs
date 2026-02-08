use anyhow::Result;
use colored::*;

use crate::api::ContentBlock;
use crate::api::Message;
use crate::providers::LLMProvider;
use crate::providers::InferenceRequest;
use crate::tools::{execute_tool, get_tool_definitions, ToolContext};

pub struct Agent {
    provider: Box<dyn LLMProvider>,
    messages: Vec<Message>,
    tool_ctx: ToolContext,
}

impl Agent {
    pub fn new(provider: Box<dyn LLMProvider>) -> Result<Self> {
        Ok(Self {
            provider,
            messages: Vec::new(),
            tool_ctx: ToolContext::new()?,
        })
    }

    pub fn add_user_message(&mut self, text: impl Into<String>) {
        self.messages.push(Message::user(text));
    }

    pub fn clear_history(&mut self) {
        self.messages.clear();
    }

    pub async fn run_turn(&mut self) -> Result<()> {
        let system_prompt = format!(
            "You are a concise coding assistant. Current working directory: {}",
            self.tool_ctx.working_dir.display()
        );

        loop {
            let req = InferenceRequest {
                model: self.provider.model().to_string(),
                messages: self.messages.clone(),
                tools: get_tool_definitions(),
                max_tokens: 8192,
                system: system_prompt.clone(),
            };

            let response = self.provider.infer(req).await?;

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
                let result = execute_tool(&name, &input, &self.tool_ctx);

                let content = match result {
                    Ok(output) => {
                        println!("  {} {}", "└─".green(), "OK".green());
                        output
                    }
                    Err(e) => {
                        let err_msg = format!("error: {e}");
                        println!("  {} {}", "└─".red(), err_msg.red());
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
