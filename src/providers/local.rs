use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

use crate::api::ContentBlock;

use super::{InferenceRequest, InferenceResponse, LLMProvider, Usage};

pub struct LocalProvider {
    client: reqwest::Client,
    host: String,
    model: String,
}

impl LocalProvider {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;

        let host =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
        let model = std::env::var("MODEL").unwrap_or_else(|_| "llama2".to_string());

        Ok(Self {
            client,
            host,
            model,
        })
    }

    pub async fn is_available() -> bool {
        let host =
            std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        {
            Ok(c) => c,
            Err(_) => return false,
        };

        match client
            .get(&format!("{}/api/tags", host))
            .send()
            .await
        {
            Ok(res) => res.status().is_success(),
            Err(_) => false,
        }
    }

    fn convert_to_ollama_message(msg: &crate::api::Message) -> Value {
        let mut content = String::new();

        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => {
                    content.push_str(text);
                }
                ContentBlock::ToolUse { id, name, input } => {
                    content.push_str(&format!(
                        "\n[TOOL_USE id={} name={}]\n{}",
                        id, name, input
                    ));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content: result_content,
                } => {
                    content.push_str(&format!(
                        "\n[TOOL_RESULT id={}]\n{}",
                        tool_use_id, result_content
                    ));
                }
            }
        }

        json!({
            "role": msg.role,
            "content": content
        })
    }
}

#[async_trait::async_trait]
impl LLMProvider for LocalProvider {
    async fn infer(&self, req: InferenceRequest) -> Result<InferenceResponse> {
        let mut messages = vec![json!({
            "role": "system",
            "content": req.system
        })];

        messages.extend(req.messages.iter().map(Self::convert_to_ollama_message));

        let body = json!({
            "model": &req.model,
            "messages": messages,
            "stream": false,
        });

        let res = self
            .client
            .post(&format!("{}/api/chat", self.host))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await?;
            anyhow::bail!("Ollama API Error {status}: {err_text}");
        }

        let response_json: Value = res.json().await?;

        let content_str = response_json
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .context("No message content in response")?;

        // Simple parsing for tool markers since local models don't support structured tools well
        let mut blocks = Vec::new();
        let mut current_text = String::new();

        for line in content_str.lines() {
            if line.starts_with("[TOOL_USE") {
                if !current_text.is_empty() {
                    blocks.push(ContentBlock::Text {
                        text: current_text.trim().to_string(),
                    });
                    current_text.clear();
                }
                // Try to parse tool use marker
                if let Some(end) = line.find(']') {
                    let marker = &line[..=end];
                    if marker.contains("id=") && marker.contains("name=") {
                        // Simplified parsing - local models don't always follow strict format
                        blocks.push(ContentBlock::Text {
                            text: marker.to_string(),
                        });
                    }
                }
            } else {
                current_text.push_str(line);
                current_text.push('\n');
            }
        }

        if !current_text.is_empty() {
            blocks.push(ContentBlock::Text {
                text: current_text.trim().to_string(),
            });
        }

        let usage = if let Some(usage_obj) = response_json.get("eval_count") {
            Usage {
                input_tokens: response_json
                    .get("prompt_eval_count")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                output_tokens: usage_obj.as_u64().unwrap_or(0) as u32,
            }
        } else {
            Usage {
                input_tokens: 0,
                output_tokens: 0,
            }
        };

        Ok(InferenceResponse {
            content: blocks,
            stop_reason: "stop".to_string(),
            usage,
        })
    }

    fn name(&self) -> &str {
        "local"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn validate_config(&self) -> Result<()> {
        if self.host.is_empty() {
            anyhow::bail!("Ollama host is empty");
        }
        Ok(())
    }

    fn supports_tool_use(&self) -> bool {
        false
    }
}
