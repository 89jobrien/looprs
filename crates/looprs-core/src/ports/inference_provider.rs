//! InferenceProvider port — abstraction over LLM inference backends.

use std::pin::Pin;

use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::api::{ContentBlock, Message, ToolDefinition};
use crate::types::ModelId;

/// A boxed async stream of text chunks from a streaming inference call.
pub type InferStream =
    Pin<Box<dyn Stream<Item = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send>>;

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

    /// Whether this provider supports token-by-token streaming.
    fn supports_streaming(&self) -> bool {
        false
    }

    /// Stream inference results as text chunks.
    ///
    /// Default implementation calls `infer()` and yields the full text as a
    /// single chunk, so all providers work without modification. Override in
    /// providers that have native SSE/streaming APIs.
    async fn infer_stream(&self, req: &InferenceRequest) -> InferStream {
        use futures::stream;

        match self.infer(req).await {
            Ok(resp) => {
                let text: String = resp
                    .content
                    .iter()
                    .filter_map(|b| {
                        if let ContentBlock::Text { text } = b {
                            Some(text.as_str())
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("");
                Box::pin(stream::once(async move { Ok(text) }))
            }
            Err(e) => Box::pin(stream::once(async move { Err(e) })),
        }
    }
}
