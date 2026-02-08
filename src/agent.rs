use anyhow::{Context, Result};
use colored::*;
use serde_json::Value;
use std::time::Duration;

use crate::api::{ApiRequest, ContentBlock, Message};
use crate::config::{ApiConfig, API_TIMEOUT_SECS};
use crate::tools::{execute_tool, get_tool_definitions, ToolContext};

pub struct Agent {
    client: reqwest::Client,
    config: ApiConfig,
    messages: Vec<Message>,
    tool_ctx: ToolContext,
}

impl Agent {
    pub fn new(config: ApiConfig) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(API_TIMEOUT_SECS))
            .build()?;

        Ok(Self {
            client,
            config,
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
            let req_body = ApiRequest {
                model: self.config.model.clone(),
                max_tokens: 8192,
                system: system_prompt.clone(),
                messages: self.messages.clone(),
                tools: get_tool_definitions(),
            };

            let res = self
                .client
                .post(&self.config.url)
                .header("x-api-key", &self.config.key)
                .header("anthropic-version", "2023-06-01")
                .header("Authorization", format!("Bearer {}", &self.config.key))
                .header("Content-Type", "application/json")
                .json(&req_body)
                .send()
                .await?;

            if !res.status().is_success() {
                let status = res.status();
                let err_text = res.text().await?;
                anyhow::bail!("API Error {status}: {err_text}");
            }

            let response_json: Value = res.json().await?;

            let content_arr = response_json["content"]
                .as_array()
                .context("Unexpected API response: missing 'content' array")?;

            let mut assistant_blocks = Vec::new();
            let mut tools_to_execute = Vec::new();

            for block in content_arr {
                match block["type"].as_str() {
                    Some("text") => {
                        let text = block["text"].as_str().unwrap_or("");
                        println!("\n{} {}", "●".blue().bold(), text.blue());
                        assistant_blocks.push(ContentBlock::Text {
                            text: text.to_string(),
                        });
                    }
                    Some("tool_use") => {
                        let id = block["id"].as_str().unwrap().to_string();
                        let name = block["name"].as_str().unwrap().to_string();
                        let input = block["input"].clone();

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

                        assistant_blocks.push(ContentBlock::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        });

                        tools_to_execute.push((id, name, input));
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
