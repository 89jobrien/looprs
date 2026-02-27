use claudius::{Anthropic, MessageCreateParams};
use serde_json::{Value, json};

use crate::api::ContentBlock;
use crate::errors::ProviderError;
use crate::types::ModelId;

use super::{InferenceRequest, InferenceResponse, LLMProvider, Usage};

pub struct AnthropicSdkProvider {
    client: Anthropic,
    key: String,
    model: ModelId,
}

impl AnthropicSdkProvider {
    pub fn new(key: String) -> Result<Self, ProviderError> {
        let model = std::env::var("MODEL").ok().map(ModelId::new);
        Self::new_with_model(key, model)
    }

    pub fn new_with_model(key: String, model: Option<ModelId>) -> Result<Self, ProviderError> {
        let model = model.unwrap_or_else(ModelId::claude_opus);
        let client = Anthropic::new(Some(key.clone()))
            .map_err(|e| ProviderError::Config(format!("Anthropic SDK setup error: {e}")))?;

        Ok(Self { client, key, model })
    }
}

#[async_trait::async_trait]
impl LLMProvider for AnthropicSdkProvider {
    async fn infer(&self, req: &InferenceRequest) -> Result<InferenceResponse, ProviderError> {
        let mut body = json!({
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

        if let Some(temp) = req.temperature {
            body["temperature"] = json!(temp);
        }

        let params: MessageCreateParams = serde_json::from_value(body)?;
        let response = self
            .client
            .send(params)
            .await
            .map_err(|e| ProviderError::ApiError(format!("Anthropic SDK Error: {e}")))?;

        let response_json: Value = serde_json::to_value(response)?;

        let content_arr = response_json["content"].as_array().ok_or_else(|| {
            ProviderError::InvalidResponse(
                "Unexpected API response: missing 'content' array".to_string(),
            )
        })?;

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
                    let id = block.get("id").and_then(|v| v.as_str()).ok_or_else(|| {
                        ProviderError::InvalidResponse("Missing tool_use id".to_string())
                    })?;
                    let name = block.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                        ProviderError::InvalidResponse("Missing tool_use name".to_string())
                    })?;
                    let input = block.get("input").cloned().ok_or_else(|| {
                        ProviderError::InvalidResponse("Missing tool_use input".to_string())
                    })?;

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
        "anthropic-sdk"
    }

    fn model(&self) -> &ModelId {
        &self.model
    }

    fn validate_config(&self) -> Result<(), ProviderError> {
        if self.key.is_empty() {
            return Err(ProviderError::Config(
                "Anthropic API key is empty".to_string(),
            ));
        }
        Ok(())
    }
}
