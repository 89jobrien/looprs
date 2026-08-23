use async_trait::async_trait;
use serde_json::Value;

use crate::api::{ContentBlock, Message, ToolDefinition as ApiToolDefinition};
use crate::baml_client::types::{
    ChatMessage, ToolCall as BamlToolCall, ToolDefinition, Union2ListToolCallOrString,
};
use crate::errors::ProviderError;
use crate::types::{ModelId, ToolId, ToolName};
use looprs_core::ports::InferenceProvider;
use looprs_core::ports::inference_provider::{InferenceRequest, InferenceResponse, Usage};

/// LLM provider backed by BAML — handles retry, fallback, and structured
/// output via the BAML runtime. Wraps the `InferenceProvider` port so the
/// agent loop is unaware of BAML internals.
pub struct BamlProvider {
    /// BAML client name: "Anthropic", "OpenAI", "Ollama", or "DefaultClient".
    client_name: String,
    model: ModelId,
}

impl BamlProvider {
    pub fn new(client_name: impl Into<String>, model: ModelId) -> Self {
        Self {
            client_name: client_name.into(),
            model,
        }
    }

    /// Convenience: select client by provider name (same naming as providers/mod.rs).
    pub fn for_provider(provider: &str, model: Option<ModelId>) -> Result<Self, ProviderError> {
        let (client_name, default_model) = match provider.to_lowercase().as_str() {
            "anthropic" | "anthropic-sdk" | "claude-sdk" => {
                ("Anthropic", ModelId::new("claude-sonnet-4-6"))
            }
            "openai" | "openai-sdk" => ("OpenAI", ModelId::new("gpt-4o")),
            "ollama" | "local" => ("Ollama", ModelId::new("llama3.2")),
            "baml" => ("DefaultClient", ModelId::new("gpt-4o")),
            other => {
                return Err(ProviderError::Config(format!(
                    "BamlProvider: unknown provider {other:?}"
                )));
            }
        };
        Ok(Self::new(client_name, model.unwrap_or(default_model)))
    }
}

/// Convert a looprs `Message` into a `ChatMessage` for BAML.
///
/// Tool use and tool result blocks are serialized as text so the conversation
/// history round-trips through BAML without losing structure. Native BAML tool
/// calling can be layered on later.
fn message_to_chat(msg: &Message) -> ChatMessage {
    let content = msg
        .content
        .iter()
        .map(|b| match b {
            ContentBlock::Text { text } => text.clone(),
            ContentBlock::ToolUse { name, input, .. } => {
                format!("[tool_use: {} {}]", name.as_str(), input)
            }
            ContentBlock::ToolResult { content, .. } => {
                format!("[tool_result: {content}]")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    ChatMessage {
        role: msg.role.clone(),
        content,
    }
}

/// Convert a looprs `ToolDefinition` into the BAML-generated equivalent,
/// serializing the JSON input schema as a string.
fn tool_to_baml(tool: &ApiToolDefinition) -> ToolDefinition {
    ToolDefinition {
        name: tool.name.clone(),
        description: tool.description.clone(),
        input_schema: tool.input_schema.to_string(),
    }
}

/// Convert a BAML-generated `ToolCall` into a looprs `ContentBlock::ToolUse`.
///
/// `arguments` is expected to be JSON-encoded; falls back to a JSON string
/// value if it fails to parse so no data is lost.
fn baml_tool_call_to_content_block(call: BamlToolCall) -> ContentBlock {
    let input: Value =
        serde_json::from_str(&call.arguments).unwrap_or_else(|_| Value::String(call.arguments));
    ContentBlock::ToolUse {
        id: ToolId::new(call.id),
        name: ToolName::new(call.name),
        input,
    }
}

#[async_trait]
impl InferenceProvider for BamlProvider {
    async fn infer(
        &self,
        req: &InferenceRequest,
    ) -> Result<InferenceResponse, Box<dyn std::error::Error + Send + Sync>> {
        use crate::baml_client::async_client::B;

        let messages: Vec<ChatMessage> = req.messages.iter().map(message_to_chat).collect();
        let tools: Vec<ToolDefinition> = req.tools.iter().map(tool_to_baml).collect();

        let collector = crate::baml_client::new_collector("baml_provider");

        let result = B
            .Chat
            .with_client(&self.client_name)
            .with_collector(&collector)
            .call(&req.system, &messages, &tools)
            .await
            .map_err(|e| ProviderError::ApiError(e.to_string()))?;

        let usage = collector.usage();
        let usage = Usage {
            input_tokens: usage.input_tokens().max(0) as u32,
            output_tokens: usage.output_tokens().max(0) as u32,
        };

        let (content, stop_reason) = match result {
            Union2ListToolCallOrString::String(text) => {
                (vec![ContentBlock::Text { text }], "end_turn".to_string())
            }
            Union2ListToolCallOrString::ListToolCall(calls) => {
                let content = calls
                    .into_iter()
                    .map(baml_tool_call_to_content_block)
                    .collect();
                (content, "tool_use".to_string())
            }
        };

        Ok(InferenceResponse {
            content,
            stop_reason,
            usage,
        })
    }

    fn name(&self) -> &str {
        "baml"
    }

    fn model(&self) -> &ModelId {
        &self.model
    }

    fn validate_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    // Tools are rendered into the prompt and the model is instructed to
    // respond with a JSON array of tool calls; BAML's union return type
    // (`string | ToolCall[]`) parses the result into structured tool calls.
    fn supports_tool_use(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use looprs_core::ports::test_contracts::assert_inference_provider_contract;

    #[test]
    fn baml_provider_satisfies_inference_provider_contract() {
        let p = BamlProvider::new("DefaultClient", ModelId::new("claude-3-5-haiku-20241022"));
        assert_inference_provider_contract(&p);
    }

    #[tokio::test]
    #[ignore = "live: set LOOPRS_RUN_LIVE_LLM_TESTS=1"]
    async fn live_contract() {
        if std::env::var("LOOPRS_RUN_LIVE_LLM_TESTS").is_err() {
            return;
        }
        let p = BamlProvider::new("Anthropic", ModelId::new("claude-3-5-haiku-20241022"));
        looprs_core::ports::test_contracts::assert_inference_provider_live_contract(&p).await;
    }

    #[tokio::test]
    #[ignore = "live: set LOOPRS_RUN_LIVE_LLM_TESTS=1"]
    async fn live_tool_use() {
        if std::env::var("LOOPRS_RUN_LIVE_LLM_TESTS").is_err() {
            return;
        }
        let p = BamlProvider::new("Anthropic", ModelId::new("claude-3-5-haiku-20241022"));
        let req = InferenceRequest {
            model: p.model.clone(),
            messages: vec![Message::user("What's the weather in Paris?")],
            tools: vec![ApiToolDefinition {
                name: "get_weather".to_string(),
                description: "Get the current weather for a city".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {"city": {"type": "string"}},
                    "required": ["city"],
                }),
            }],
            max_tokens: 1024,
            temperature: None,
            system: "You are a helpful assistant.".to_string(),
        };

        let resp = p.infer(&req).await.expect("live inference should succeed");
        assert!(!resp.content.is_empty());
    }

    #[test]
    fn supports_tool_use_is_true() {
        let p = BamlProvider::new("DefaultClient", ModelId::new("claude-3-5-haiku-20241022"));
        assert!(p.supports_tool_use());
    }

    #[test]
    fn tool_to_baml_serializes_input_schema() {
        let tool = ApiToolDefinition {
            name: "read".to_string(),
            description: "Read a file".to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        };
        let baml_tool = tool_to_baml(&tool);
        assert_eq!(baml_tool.name, "read");
        assert_eq!(baml_tool.description, "Read a file");
        assert_eq!(baml_tool.input_schema, r#"{"type":"object"}"#);
    }

    #[test]
    fn baml_tool_call_to_content_block_parses_json_arguments() {
        let call = BamlToolCall {
            name: "read".to_string(),
            arguments: r#"{"path":"/tmp/file"}"#.to_string(),
            id: "call_1".to_string(),
        };
        let block = baml_tool_call_to_content_block(call);
        match block {
            ContentBlock::ToolUse { id, name, input } => {
                assert_eq!(id.as_str(), "call_1");
                assert_eq!(name.as_str(), "read");
                assert_eq!(input["path"], "/tmp/file");
            }
            other => panic!("expected ToolUse block, got {other:?}"),
        }
    }

    #[test]
    fn baml_tool_call_to_content_block_falls_back_on_invalid_json() {
        let call = BamlToolCall {
            name: "read".to_string(),
            arguments: "not json".to_string(),
            id: "call_2".to_string(),
        };
        let block = baml_tool_call_to_content_block(call);
        match block {
            ContentBlock::ToolUse { input, .. } => {
                assert_eq!(input, Value::String("not json".to_string()));
            }
            other => panic!("expected ToolUse block, got {other:?}"),
        }
    }

    #[test]
    fn union_string_variant_maps_to_text_content_block() {
        let union = Union2ListToolCallOrString::String("hello".to_string());
        let (content, stop_reason) = match union {
            Union2ListToolCallOrString::String(text) => {
                (vec![ContentBlock::Text { text }], "end_turn".to_string())
            }
            Union2ListToolCallOrString::ListToolCall(calls) => {
                let content = calls
                    .into_iter()
                    .map(baml_tool_call_to_content_block)
                    .collect();
                (content, "tool_use".to_string())
            }
        };
        assert_eq!(stop_reason, "end_turn");
        match &content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "hello"),
            other => panic!("expected Text block, got {other:?}"),
        }
    }

    #[test]
    fn union_tool_call_variant_maps_to_tool_use_content_blocks() {
        let union = Union2ListToolCallOrString::ListToolCall(vec![BamlToolCall {
            name: "read".to_string(),
            arguments: r#"{"path":"a.txt"}"#.to_string(),
            id: "call_1".to_string(),
        }]);
        let (content, stop_reason) = match union {
            Union2ListToolCallOrString::String(text) => {
                (vec![ContentBlock::Text { text }], "end_turn".to_string())
            }
            Union2ListToolCallOrString::ListToolCall(calls) => {
                let content = calls
                    .into_iter()
                    .map(baml_tool_call_to_content_block)
                    .collect();
                (content, "tool_use".to_string())
            }
        };
        assert_eq!(stop_reason, "tool_use");
        assert_eq!(content.len(), 1);
        match &content[0] {
            ContentBlock::ToolUse { name, .. } => assert_eq!(name.as_str(), "read"),
            other => panic!("expected ToolUse block, got {other:?}"),
        }
    }
}
