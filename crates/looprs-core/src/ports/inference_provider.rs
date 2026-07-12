//! InferenceProvider port — abstraction over LLM inference backends.

use serde::{Deserialize, Serialize};

use crate::api::{ContentBlock, Message, ToolDefinition};
use crate::types::ModelId;

/// Request structure for LLM inference.
#[derive(Debug, Clone)]
pub struct InferenceRequest {
    pub model: ModelId,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolDefinition>,
    pub max_tokens: u32,
    pub temperature: Option<f32>,
    pub system: String,
}

/// Response structure from LLM inference.
#[derive(Debug, Clone)]
pub struct InferenceResponse {
    pub content: Vec<ContentBlock>,
    pub stop_reason: String,
    pub usage: Usage,
}

/// Token usage information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// Port: perform LLM inference.
///
/// Implementations decide the backend (Anthropic, OpenAI, local Ollama, etc.).
#[async_trait::async_trait]
// TODO: add provider conformance test suite — a shared test matrix exercising
// every InferenceProvider implementation: correct tool-use round-trip, retry on
// 429, model name normalisation, timeout propagation. Wire via
// assert_inference_provider_contract() in test_contracts.rs. Live tests already
// opt-in via LOOPRS_RUN_LIVE_LLM_TESTS=1.
pub trait InferenceProvider: Send + Sync {
    /// Run inference with the given request.
    async fn infer(
        &self,
        req: &InferenceRequest,
    ) -> Result<InferenceResponse, Box<dyn std::error::Error + Send + Sync>>;

    /// Get the name of this provider.
    fn name(&self) -> &str;

    /// Get the model being used.
    fn model(&self) -> &ModelId;

    /// Validate that this provider is properly configured.
    fn validate_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    /// Whether this provider supports tool use (function calling).
    fn supports_tool_use(&self) -> bool {
        true
    }
}
