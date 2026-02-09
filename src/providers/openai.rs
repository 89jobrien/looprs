use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::time::Duration;

use crate::api::ContentBlock;

use super::{InferenceRequest, InferenceResponse, LLMProvider, Usage};

pub struct OpenAIProvider {
    client: reqwest::Client,
    key: String,
    model: String,
}

impl OpenAIProvider {
    pub fn new(key: String) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()?;

        let model = std::env::var("MODEL").unwrap_or_else(|_| "gpt-5.2".to_string());

        Ok(Self { client, key, model })
    }

    fn convert_to_openai_message(msg: &crate::api::Message) -> Value {
        let mut content = Vec::new();

        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => {
                    content.push(json!({
                        "type": "text",
                        "text": text
                    }));
                }
                ContentBlock::ToolUse { id, name, input } => {
                    content.push(json!({
                        "type": "tool_use",
                        "id": id,
                        "name": name,
                        "input": input
                    }));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content: result_content,
                } => {
                    content.push(json!({
                        "type": "tool_result",
                        "tool_use_id": tool_use_id,
                        "content": result_content
                    }));
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
impl LLMProvider for OpenAIProvider {
    async fn infer(&self, req: InferenceRequest) -> Result<InferenceResponse> {
        let tools = req
            .tools
            .iter()
            .map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.input_schema,
                    }
                })
            })
            .collect::<Vec<_>>();

        let body = json!({
            "model": &req.model,
            "max_tokens": req.max_tokens,
            "messages": vec![
                json!({
                    "role": "system",
                    "content": req.system
                })
            ]
            .into_iter()
            .chain(req.messages.iter().map(Self::convert_to_openai_message))
            .collect::<Vec<_>>(),
            "tools": tools,
            "tool_choice": if tools.is_empty() { "none" } else { "auto" }
        });

        let res = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await?;
            anyhow::bail!("OpenAI API Error {status}: {err_text}");
        }

        let response_json: Value = res.json().await?;

        let choice = response_json
            .get("choices")
            .and_then(|arr| arr.as_array())
            .and_then(|arr| arr.first())
            .context("No choices in response")?;

        let message = choice.get("message").context("No message in choice")?;

        let mut blocks = Vec::new();

        if let Some(text) = message.get("content").and_then(|v| v.as_str()) {
            if !text.is_empty() {
                blocks.push(ContentBlock::Text {
                    text: text.to_string(),
                });
            }
        }

        if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
            for tool_call in tool_calls {
                if let (Some(id), Some(function)) = (tool_call.get("id"), tool_call.get("function"))
                {
                    if let (Some(name), Some(args)) =
                        (function.get("name"), function.get("arguments"))
                    {
                        let args_str = if let Some(s) = args.as_str() {
                            serde_json::from_str(s).unwrap_or(args.clone())
                        } else {
                            args.clone()
                        };

                        blocks.push(ContentBlock::ToolUse {
                            id: id.as_str().unwrap_or("").to_string(),
                            name: name.as_str().unwrap_or("").to_string(),
                            input: args_str,
                        });
                    }
                }
            }
        }

        let stop_reason = choice
            .get("finish_reason")
            .and_then(|v| v.as_str())
            .unwrap_or("stop")
            .to_string();

        let usage = if let Some(usage_obj) = response_json.get("usage") {
            Usage {
                input_tokens: usage_obj
                    .get("prompt_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                output_tokens: usage_obj
                    .get("completion_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
            }
        } else {
            Usage {
                input_tokens: 0,
                output_tokens: 0,
            }
        };

        Ok(InferenceResponse {
            content: blocks,
            stop_reason,
            usage,
        })
    }

    fn name(&self) -> &str {
        "openai"
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn validate_config(&self) -> Result<()> {
        if self.key.is_empty() {
            anyhow::bail!("OpenAI API key is empty");
        }
        Ok(())
    }
}
