use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::api::ContentBlock;

use super::{InferenceRequest, InferenceResponse, LLMProvider, ProviderHttpClient, Usage};
use crate::types::ModelId;

pub struct AnthropicProvider {
    http: ProviderHttpClient,
    key: String,
    model: ModelId,
}

impl AnthropicProvider {
    pub fn new(key: String) -> Result<Self> {
        let model = std::env::var("MODEL").ok().map(ModelId::new);
        Self::new_with_model(key, model)
    }

    pub fn new_with_model(key: String, model: Option<ModelId>) -> Result<Self> {
        let http = ProviderHttpClient::default()?;

        let model = model.unwrap_or_else(ModelId::claude_opus);

        Ok(Self { http, key, model })
    }
}

#[async_trait::async_trait]
impl LLMProvider for AnthropicProvider {
    async fn infer(&self, req: &InferenceRequest) -> Result<InferenceResponse> {
        let body = json!({
            "model": req.model.as_str(),
            "max_tokens": req.max_tokens,
            "system": req.system,
            "messages": req.messages,
            "tools": req.tools
                .iter()
                .map(|t| json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.input_schema,
                }))
                .collect::<Vec<_>>(),
        });

        let res = self
            .http
            .client()
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await?;
            anyhow::bail!("Anthropic API Error {status}: {err_text}");
        }

        let response_json: Value = res.json().await?;

        let content_arr = response_json["content"]
            .as_array()
            .context("Unexpected API response: missing 'content' array")?;

        let mut blocks = Vec::new();

        for block in content_arr {
            let block_type = block.get("type").and_then(|v| v.as_str());

            match block_type {
                Some("text") => {
                    if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                        blocks.push(ContentBlock::Text {
                            text: text.to_string(),
                        });
                    }
                }
                Some("tool_use") => {
                    let id = block
                        .get("id")
                        .and_then(|v| v.as_str())
                        .context("Missing tool_use id")?;
                    let name = block
                        .get("name")
                        .and_then(|v| v.as_str())
                        .context("Missing tool_use name")?;
                    let input = block
                        .get("input")
                        .cloned()
                        .context("Missing tool_use input")?;

                    blocks.push(ContentBlock::ToolUse {
                        id: crate::types::ToolId::new(id),
                        name: crate::types::ToolName::new(name),
                        input,
                    });
                }
                _ => {}
            }
        }

        let stop_reason = response_json
            .get("stop_reason")
            .and_then(|v| v.as_str())
            .unwrap_or("end_turn")
            .to_string();

        let usage = if let Some(usage_obj) = response_json.get("usage") {
            Usage {
                input_tokens: usage_obj
                    .get("input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                output_tokens: usage_obj
                    .get("output_tokens")
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
        "anthropic"
    }

    fn model(&self) -> &ModelId {
        &self.model
    }

    fn validate_config(&self) -> Result<()> {
        if self.key.is_empty() {
            anyhow::bail!("Anthropic API key is empty");
        }
        Ok(())
    }
}
