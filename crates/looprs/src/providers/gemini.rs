use serde_json::{Value, json};

use crate::api::ContentBlock;
use crate::errors::ProviderError;

use super::{InferenceRequest, InferenceResponse, LLMProvider, ProviderHttpClient, Usage};
use crate::types::ModelId;

const GEMINI_BASE_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions";

pub struct GeminiProvider {
    http: ProviderHttpClient,
    key: String,
    model: ModelId,
}

impl GeminiProvider {
    pub fn new(key: String) -> Result<Self, ProviderError> {
        let model = std::env::var("MODEL").ok().map(ModelId::new);
        Self::new_with_model(key, model)
    }

    pub fn new_with_model(key: String, model: Option<ModelId>) -> Result<Self, ProviderError> {
        let http = ProviderHttpClient::default()?;
        let model = model.unwrap_or_else(|| ModelId::new("gemini-2.0-flash"));
        Ok(Self { http, key, model })
    }
}

#[async_trait::async_trait]
impl LLMProvider for GeminiProvider {
    async fn infer(
        &self,
        req: &InferenceRequest,
    ) -> Result<InferenceResponse, Box<dyn std::error::Error + Send + Sync>> {
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

        let mut messages: Vec<Value> = vec![json!({
            "role": "system",
            "content": req.system
        })];
        messages.extend(
            req.messages
                .iter()
                .flat_map(super::convert_to_openai_messages),
        );

        let mut body = json!({
            "model": req.model.as_str(),
            "messages": messages,
            "max_tokens": req.max_tokens,
        });

        if !tools.is_empty() {
            body["tools"] = json!(tools);
            body["tool_choice"] = json!("auto");
        }

        if let Some(temp) = req.temperature {
            body["temperature"] = json!(temp);
        }

        let res = self
            .http
            .client()
            .post(GEMINI_BASE_URL)
            .bearer_auth(&self.key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await?;
            return Err(
                ProviderError::ApiError(format!("Gemini API Error {status}: {err_text}")).into(),
            );
        }

        let response_json: Value = res.json().await?;

        let choice = response_json
            .get("choices")
            .and_then(|arr| arr.as_array())
            .and_then(|arr| arr.first())
            .ok_or_else(|| ProviderError::InvalidResponse("No choices in response".to_string()))?;

        let message = choice
            .get("message")
            .ok_or_else(|| ProviderError::InvalidResponse("No message in choice".to_string()))?;

        let mut blocks = Vec::new();

        if let Some(text) = message.get("content").and_then(|v| v.as_str())
            && !text.is_empty()
        {
            blocks.push(ContentBlock::Text {
                text: text.to_string(),
            });
        }

        if let Some(tool_calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
            for tool_call in tool_calls {
                if let (Some(id), Some(function)) = (tool_call.get("id"), tool_call.get("function"))
                    && let (Some(name), Some(args)) =
                        (function.get("name"), function.get("arguments"))
                {
                    let args_str = if let Some(s) = args.as_str() {
                        serde_json::from_str(s).unwrap_or(args.clone())
                    } else {
                        args.clone()
                    };
                    blocks.push(ContentBlock::ToolUse {
                        id: crate::types::ToolId::new(id.as_str().unwrap_or("")),
                        name: crate::types::ToolName::new(name.as_str().unwrap_or("")),
                        input: args_str,
                    });
                }
            }
        }

        let stop_reason = choice
            .get("finish_reason")
            .and_then(|v| v.as_str())
            .unwrap_or("stop")
            .to_string();

        let usage = if let Some(u) = response_json.get("usage") {
            Usage {
                input_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                output_tokens: u
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
        "gemini"
    }

    fn model(&self) -> &ModelId {
        &self.model
    }

    fn validate_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.key.is_empty() {
            return Err(ProviderError::Config("Gemini API key is empty".to_string()).into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use looprs_core::ports::test_contracts::assert_inference_provider_contract;

    #[test]
    fn gemini_provider_satisfies_inference_provider_contract() {
        let p = GeminiProvider::new("test-key".to_string())
            .expect("GeminiProvider::new must succeed in test");
        assert_inference_provider_contract(&p);
    }
}
