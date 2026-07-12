use async_trait::async_trait;

use crate::api::{ContentBlock, Message};
use crate::baml_client::types::ChatMessage;
use crate::errors::ProviderError;
use crate::types::ModelId;
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
            "baml" => ("DefaultClient", ModelId::new("claude-sonnet-4-6")),
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

#[async_trait]
impl InferenceProvider for BamlProvider {
    async fn infer(
        &self,
        req: &InferenceRequest,
    ) -> Result<InferenceResponse, Box<dyn std::error::Error + Send + Sync>> {
        use crate::baml_client::async_client::B;

        let messages: Vec<ChatMessage> = req.messages.iter().map(message_to_chat).collect();

        let text = B
            .Chat
            .with_client(&self.client_name)
            .call(&req.system, &messages)
            .await
            .map_err(|e| ProviderError::ApiError(e.to_string()))?;

        Ok(InferenceResponse {
            content: vec![ContentBlock::Text { text }],
            stop_reason: "end_turn".to_string(),
            usage: Usage {
                input_tokens: 0,
                output_tokens: 0,
            },
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

    // BAML handles tool serialization as text for now; native tool calling
    // can be added via ClientRegistry when needed.
    fn supports_tool_use(&self) -> bool {
        false
    }
}
