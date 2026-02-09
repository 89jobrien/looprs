use serde_json::{Value, json};

use crate::api::ContentBlock;
use crate::errors::ProviderError;

use super::{InferenceRequest, InferenceResponse, LLMProvider, ProviderHttpClient, Usage};
use crate::types::ModelId;

/// OpenAI provider implementation
/// 
/// API differences:
/// - GPT-5.x and newer GPT-4 models use `max_completion_tokens` instead of `max_tokens`
/// - Tool calls use OpenAI's function calling format (different from Anthropic)
/// - System messages are passed as a separate message in the messages array
pub struct OpenAIProvider {
    http: ProviderHttpClient,
    key: String,
    model: ModelId,
}

impl OpenAIProvider {
    pub fn new(key: String) -> Result<Self, ProviderError> {
        let model = std::env::var("MODEL").ok().map(ModelId::new);
        Self::new_with_model(key, model)
    }

    pub fn new_with_model(key: String, model: Option<ModelId>) -> Result<Self, ProviderError> {
        let http = ProviderHttpClient::default()?;

        let model = model.unwrap_or_else(ModelId::gpt_5_mini);

        Ok(Self { http, key, model })
    }

    fn convert_to_openai_messages(msg: &crate::api::Message) -> Vec<Value> {
        let mut messages = Vec::new();
        let mut text_parts = Vec::new();
        let mut tool_calls = Vec::new();
        
        // Separate content into text, tool uses, and tool results
        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => {
                    text_parts.push(text.clone());
                }
                ContentBlock::ToolUse { id, name, input } => {
                    // OpenAI format: tool_calls array in assistant message
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
                    // OpenAI format: separate message with role "tool"
                    messages.push(json!({
                        "role": "tool",
                        "tool_call_id": tool_use_id.as_str(),
                        "content": result_content
                    }));
                }
            }
        }
        
        // Build the main message if there's text or tool calls
        if !text_parts.is_empty() || !tool_calls.is_empty() {
            let mut main_msg = json!({
                "role": msg.role,
            });
            
            // Add content if we have text
            if !text_parts.is_empty() {
                main_msg["content"] = json!(text_parts.join("\n"));
            } else if tool_calls.is_empty() {
                // OpenAI requires content field if no tool_calls
                main_msg["content"] = json!("");
            }
            
            // Add tool_calls if we have any
            if !tool_calls.is_empty() {
                main_msg["tool_calls"] = json!(tool_calls);
            }
            
            messages.insert(0, main_msg);
        }
        
        messages
    }
}

#[async_trait::async_trait]
impl LLMProvider for OpenAIProvider {
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

        // GPT-5+ and newer GPT-4 models use max_completion_tokens instead of max_tokens
        let model = req.model.as_str();
        let uses_completion_tokens = model.starts_with("gpt-5")
            || model.starts_with("gpt-4o")
            || model.starts_with("gpt-4-turbo-2024");

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

        // Use the correct parameter name based on model
        if uses_completion_tokens {
            body["max_completion_tokens"] = json!(req.max_tokens);
        } else {
            body["max_tokens"] = json!(req.max_tokens);
        }
        if let Some(temp) = req.temperature {
            body["temperature"] = json!(temp);
        }

        let res = self
            .http
            .client()
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await?;
            return Err(ProviderError::ApiError(format!(
                "OpenAI API Error {status}: {err_text}"
            )));
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
                            id: crate::types::ToolId::new(id.as_str().unwrap_or("")),
                            name: crate::types::ToolName::new(name.as_str().unwrap_or("")),
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
