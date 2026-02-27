use async_openai::Client;
use async_openai::config::OpenAIConfig;
use serde_json::{Value, json};

use crate::api::ContentBlock;
use crate::errors::ProviderError;
use crate::types::ModelId;

use super::{InferenceRequest, InferenceResponse, LLMProvider, Usage};

pub struct OpenAISdkProvider {
    client: Client<OpenAIConfig>,
    key: String,
    model: ModelId,
}

impl OpenAISdkProvider {
    pub fn new(key: String) -> Result<Self, ProviderError> {
        let model = std::env::var("MODEL").ok().map(ModelId::new);
        Self::new_with_model(key, model)
    }

    pub fn new_with_model(key: String, model: Option<ModelId>) -> Result<Self, ProviderError> {
        let model = model.unwrap_or_else(ModelId::gpt_5_mini);
        let config = OpenAIConfig::new().with_api_key(&key);
        let client = Client::with_config(config);
        Ok(Self { client, key, model })
    }

    fn is_reasoning_model(model: &str) -> bool {
        model.starts_with("o1") || model.starts_with("o3")
    }

    fn supports_temperature(model: &str) -> bool {
        !Self::is_reasoning_model(model) && !model.starts_with("gpt-5")
    }

    fn convert_to_openai_messages(msg: &crate::api::Message) -> Vec<Value> {
        let mut messages = Vec::new();
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();

        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => {
                    text_parts.push(text.clone());
                }
                ContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(json!({
                        "id": id.as_str(),
                        "type": "function",
                        "function": {
                            "name": name.as_str(),
                            "arguments": serde_json::to_string(input).unwrap_or_default()
                        }
                    }));
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content: result_content,
                } => {
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tool_use_id.as_str(),
                        "content": result_content
                    }));
                }
            }
        }

        if !text_parts.is_empty() || !tool_calls.is_empty() {
            let mut main_msg = json!({
                "role": msg.role,
            });

            if !text_parts.is_empty() {
                main_msg["content"] = json!(text_parts.join("\n"));
            } else if tool_calls.is_empty() {
                main_msg["content"] = json!("");
            }

            if !tool_calls.is_empty() {
                main_msg["tool_calls"] = json!(tool_calls);
            }

            messages.insert(0, main_msg);
        }

        messages
    }

    fn parse_tool_arguments(args: &Value) -> Result<Value, ProviderError> {
        match args {
            Value::String(raw) => serde_json::from_str(raw).map_err(|e| {
                ProviderError::InvalidResponse(format!("Invalid tool call arguments JSON: {e}"))
            }),
            other => Ok(other.clone()),
        }
    }

    fn parse_tool_call(tool_call: &Value) -> Result<ContentBlock, ProviderError> {
        let id = tool_call
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                ProviderError::InvalidResponse("Tool call id missing or invalid".to_string())
            })?;

        let function = tool_call.get("function").ok_or_else(|| {
            ProviderError::InvalidResponse("Tool call function missing".to_string())
        })?;

        let name = function
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                ProviderError::InvalidResponse(
                    "Tool call function name missing or invalid".to_string(),
                )
            })?;

        let args = function.get("arguments").ok_or_else(|| {
            ProviderError::InvalidResponse("Tool call arguments missing".to_string())
        })?;

        let input = Self::parse_tool_arguments(args)?;

        Ok(ContentBlock::ToolUse {
            id: crate::types::ToolId::new(id),
            name: crate::types::ToolName::new(name),
            input,
        })
    }
}

#[async_trait::async_trait]
impl LLMProvider for OpenAISdkProvider {
    async fn infer(&self, req: &InferenceRequest) -> Result<InferenceResponse, ProviderError> {
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

        let model = req.model.as_str();
        let is_reasoning = Self::is_reasoning_model(model);
        let uses_completion_tokens = model.starts_with("gpt-5")
            || model.starts_with("gpt-4o")
            || model.starts_with("gpt-4-turbo-2024")
            || is_reasoning;

        let mut body = json!({
            "model": req.model.as_str(),
            "messages": vec![
                json!({
                    "role": "system",
                    "content": req.system
                })
            ]
            .into_iter()
            .chain(req.messages.iter().flat_map(Self::convert_to_openai_messages))
            .collect::<Vec<_>>(),
            "tools": tools,
            "tool_choice": if tools.is_empty() { "none" } else { "auto" }
        });

        if uses_completion_tokens {
            body["max_completion_tokens"] = json!(req.max_tokens);
        } else {
            body["max_tokens"] = json!(req.max_tokens);
        }

        if Self::supports_temperature(model)
            && let Some(temp) = req.temperature
        {
            body["temperature"] = json!(temp);
        }

        let response_json: Value = self
            .client
            .chat()
            .create_byot(body)
            .await
            .map_err(|e| ProviderError::ApiError(format!("OpenAI SDK Error: {e}")))?;

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
                blocks.push(Self::parse_tool_call(tool_call)?);
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
        "openai-sdk"
    }

    fn model(&self) -> &ModelId {
        &self.model
    }

    fn validate_config(&self) -> Result<(), ProviderError> {
        if self.key.is_empty() {
            return Err(ProviderError::Config("OpenAI API key is empty".to_string()));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tool_call_rejects_missing_id() {
        let tool_call = json!({
            "function": {
                "name": "read",
                "arguments": "{\"path\":\"README.md\"}"
            }
        });

        let err = OpenAISdkProvider::parse_tool_call(&tool_call).unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse(_)));
        assert!(err.to_string().contains("Tool call id missing or invalid"));
    }

    #[test]
    fn parse_tool_call_rejects_invalid_json_arguments() {
        let tool_call = json!({
            "id": "call_1",
            "function": {
                "name": "read",
                "arguments": "{not json}"
            }
        });

        let err = OpenAISdkProvider::parse_tool_call(&tool_call).unwrap_err();
        assert!(matches!(err, ProviderError::InvalidResponse(_)));
        assert!(err.to_string().contains("Invalid tool call arguments JSON"));
    }

    #[test]
    fn parse_tool_call_accepts_valid_payload() {
        let tool_call = json!({
            "id": "call_2",
            "function": {
                "name": "read",
                "arguments": "{\"path\":\"README.md\"}"
            }
        });

        let parsed = OpenAISdkProvider::parse_tool_call(&tool_call).unwrap();

        match parsed {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id.as_str(), "call_2");
                assert_eq!(name.as_str(), "read");
                assert_eq!(input.get("path").and_then(Value::as_str), Some("README.md"));
            }
            _ => panic!("expected tool use block"),
        }
    }
}
