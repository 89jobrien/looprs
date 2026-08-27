use async_openai::Client;
use async_openai::config::OpenAIConfig;
use futures::{StreamExt, stream};
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

    fn convert_to_openai_messages(msg: &crate::api::Message) -> Vec<Value> {
        super::convert_to_openai_messages(msg)
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

    fn extract_stream_text(payload: &Value) -> Option<String> {
        let choices = payload.get("choices")?.as_array()?;
        let first = choices.first()?;
        let delta = first.get("delta")?;
        let text = delta.get("content")?.as_str()?;
        if text.is_empty() {
            return None;
        }
        Some(text.to_string())
    }
}

#[async_trait::async_trait]
impl LLMProvider for OpenAISdkProvider {
    // qual:allow(iosp) reason: "I/O boundary — builds and sends HTTP request to OpenAI API"
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

        let model = req.model.as_str();
        let is_reasoning = super::is_reasoning_model(model);
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

        if super::supports_temperature(model)
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

    fn supports_streaming(&self) -> bool {
        true
    }

    async fn infer_stream(&self, req: &InferenceRequest) -> looprs_core::ports::InferStream {
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
        let is_reasoning = super::is_reasoning_model(model);
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
            "tool_choice": if tools.is_empty() { "none" } else { "auto" },
            "stream": true,
        });

        if uses_completion_tokens {
            body["max_completion_tokens"] = json!(req.max_tokens);
        } else {
            body["max_tokens"] = json!(req.max_tokens);
        }

        if super::supports_temperature(model)
            && let Some(temp) = req.temperature
        {
            body["temperature"] = json!(temp);
        }

        type JsonStream = std::pin::Pin<
            Box<dyn futures::Stream<Item = Result<Value, async_openai::error::OpenAIError>> + Send>,
        >;

        let stream_result: Result<JsonStream, async_openai::error::OpenAIError> =
            self.client.chat().create_stream_byot(body).await;

        let response_stream = match stream_result {
            Ok(s) => s,
            Err(e) => {
                return Box::pin(stream::once(async move {
                    Err(Box::new(ProviderError::ApiError(format!(
                        "OpenAI SDK stream error: {e}"
                    )))
                        as Box<dyn std::error::Error + Send + Sync>)
                }));
            }
        };

        let text_stream = response_stream.flat_map(|item| {
            let maybe_text: Option<Result<String, Box<dyn std::error::Error + Send + Sync>>> =
                match item {
                    Ok(payload) => Self::extract_stream_text(&payload).map(Ok),
                    Err(e) => Some(Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)),
                };
            stream::iter(maybe_text)
        });

        Box::pin(text_stream)
    }

    fn model(&self) -> &ModelId {
        &self.model
    }

    fn validate_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.key.is_empty() {
            return Err(ProviderError::Config("OpenAI API key is empty".to_string()).into());
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

    #[test]
    fn openai_sdk_provider_reports_streaming_support() {
        let p = OpenAISdkProvider::new("test-key".to_string())
            .expect("OpenAISdkProvider::new must succeed in test");
        assert!(p.supports_streaming());
    }

    #[test]
    fn extract_stream_text_reads_delta_content() {
        let payload = json!({
            "choices": [
                {
                    "delta": {
                        "content": "hello"
                    }
                }
            ]
        });

        assert_eq!(
            OpenAISdkProvider::extract_stream_text(&payload),
            Some("hello".to_string())
        );
    }

    #[test]
    fn extract_stream_text_ignores_empty_or_missing_delta() {
        let empty = json!({
            "choices": [
                {
                    "delta": {
                        "content": ""
                    }
                }
            ]
        });
        let missing = json!({ "choices": [] });

        assert_eq!(OpenAISdkProvider::extract_stream_text(&empty), None);
        assert_eq!(OpenAISdkProvider::extract_stream_text(&missing), None);
    }

    #[test]
    fn openai_sdk_provider_satisfies_inference_provider_contract() {
        use looprs_core::ports::test_contracts::assert_inference_provider_contract;
        let p = OpenAISdkProvider::new("test-key".to_string())
            .expect("OpenAISdkProvider::new must succeed in test");
        assert_inference_provider_contract(&p);
    }

    #[tokio::test]
    #[ignore = "live: set LOOPRS_RUN_LIVE_LLM_TESTS=1"]
    async fn live_contract() {
        if std::env::var("LOOPRS_RUN_LIVE_LLM_TESTS").is_err() {
            return;
        }
        let key = match std::env::var("OPENAI_API_KEY") {
            Ok(k) => k,
            Err(_) => return,
        };
        let p = OpenAISdkProvider::new(key).expect("OpenAISdkProvider::new must succeed");
        looprs_core::ports::test_contracts::assert_inference_provider_live_contract(&p).await;
    }
}
