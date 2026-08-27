use futures::{StreamExt, stream};
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
        super::convert_to_openai_messages(msg)
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
impl LLMProvider for OpenAIProvider {
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

        // GPT-5+, newer GPT-4, and reasoning models use max_completion_tokens instead of max_tokens
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

        // Use the correct parameter name based on model
        if uses_completion_tokens {
            body["max_completion_tokens"] = json!(req.max_tokens);
        } else {
            body["max_tokens"] = json!(req.max_tokens);
        }

        // Only send temperature if the model supports it and user specified it
        if super::supports_temperature(model)
            && let Some(temp) = req.temperature
        {
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
            return Err(
                ProviderError::ApiError(format!("OpenAI API Error {status}: {err_text}")).into(),
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

        let result = self
            .http
            .client()
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await;

        let resp = match result {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let status = r.status();
                let err_text = r.text().await.unwrap_or_default();
                return Box::pin(stream::once(async move {
                    Err(Box::new(ProviderError::ApiError(format!(
                        "OpenAI API Error {status}: {err_text}"
                    )))
                        as Box<dyn std::error::Error + Send + Sync>)
                }));
            }
            Err(e) => {
                return Box::pin(stream::once(async move {
                    Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                }));
            }
        };

        let byte_stream = resp.bytes_stream();
        let text_stream = byte_stream.flat_map(|chunk_result| {
            let lines: Vec<Result<String, Box<dyn std::error::Error + Send + Sync>>> =
                match chunk_result {
                    Err(e) => vec![Err(Box::new(e) as _)],
                    Ok(bytes) => {
                        let raw = String::from_utf8_lossy(&bytes);
                        raw.lines()
                            .filter_map(|line| {
                                let data = line.strip_prefix("data: ")?;
                                if data == "[DONE]" {
                                    return None;
                                }
                                let payload: Value = serde_json::from_str(data).ok()?;
                                Self::extract_stream_text(&payload).map(Ok)
                            })
                            .collect()
                    }
                };
            stream::iter(lines)
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
    use super::OpenAIProvider;
    use crate::providers::LLMProvider;
    use serde_json::json;

    #[test]
    fn test_is_reasoning_model() {
        assert!(crate::providers::is_reasoning_model("o1-preview"));
        assert!(crate::providers::is_reasoning_model("o1-mini"));
        assert!(crate::providers::is_reasoning_model("o3-mini"));
        assert!(!crate::providers::is_reasoning_model("gpt-4"));
        assert!(!crate::providers::is_reasoning_model("gpt-5"));
    }

    #[test]
    fn test_supports_temperature() {
        assert!(!crate::providers::supports_temperature("o1-preview"));
        assert!(!crate::providers::supports_temperature("o1-mini"));
        assert!(!crate::providers::supports_temperature("o3-mini"));
        assert!(crate::providers::supports_temperature("gpt-4"));
        assert!(!crate::providers::supports_temperature("gpt-5"));
        assert!(!crate::providers::supports_temperature("gpt-5-mini"));
        assert!(crate::providers::supports_temperature("gpt-4o"));
    }

    #[test]
    fn openai_provider_reports_streaming_support() {
        let p = OpenAIProvider::new("test-key".to_string())
            .expect("OpenAIProvider::new must succeed in test");
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
            OpenAIProvider::extract_stream_text(&payload),
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

        assert_eq!(OpenAIProvider::extract_stream_text(&empty), None);
        assert_eq!(OpenAIProvider::extract_stream_text(&missing), None);
    }

    #[test]
    fn openai_provider_satisfies_inference_provider_contract() {
        use crate::providers::openai::OpenAIProvider;
        use looprs_core::ports::test_contracts::assert_inference_provider_contract;
        let p = OpenAIProvider::new("test-key".to_string())
            .expect("OpenAIProvider::new must succeed in test");
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
        use crate::providers::openai::OpenAIProvider;
        let p = OpenAIProvider::new(key).expect("OpenAIProvider::new must succeed");
        looprs_core::ports::test_contracts::assert_inference_provider_live_contract(&p).await;
    }
}
