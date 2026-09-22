//! InferenceProvider port — abstraction over LLM inference backends.

use std::pin::Pin;

use futures::Stream;
use serde::{Deserialize, Serialize};

use crate::api::{ContentBlock, Message, ToolDefinition};
use crate::types::ModelId;

/// Error returned while producing a streaming inference response.
pub type InferenceStreamError = Box<dyn std::error::Error + Send + Sync>;

/// A typed incremental update from a streaming provider.
#[derive(Debug, Clone)]
pub enum InferenceDelta {
    /// Assistant text suitable for immediate display.
    Text(String),
    /// One incremental tool-call update from the provider.
    ToolCall {
        /// Provider-assigned tool-call position within the response.
        index: usize,
        /// Tool-use identifier when supplied by this update.
        id: Option<String>,
        /// Tool name when supplied by this update.
        name: Option<String>,
        /// JSON argument fragment supplied by this update.
        arguments_fragment: String,
    },
}

/// One event from a streaming inference request.
#[derive(Debug, Clone)]
pub enum InferenceStreamEvent {
    /// An incremental update that may be rendered before completion.
    Delta(InferenceDelta),
    /// The authoritative structured response for this request.
    Final(InferenceResponse),
}

/// A boxed async stream containing typed deltas and one terminal response.
pub type InferStream =
    Pin<Box<dyn Stream<Item = Result<InferenceStreamEvent, InferenceStreamError>> + Send>>;

/// Request structure for LLM inference.
#[derive(Debug, Clone)]
pub struct InferenceRequest {
    /// Target model identifier.
    pub model: ModelId,
    /// Full message history for this inference call.
    pub messages: Vec<Message>,
    /// Tool/function definitions exposed to the provider.
    pub tools: Vec<ToolDefinition>,
    /// Output token cap for the response.
    pub max_tokens: u32,
    /// Sampling temperature override, if set.
    pub temperature: Option<f32>,
    /// System prompt used for this call.
    pub system: String,
}

/// Response structure from LLM inference.
#[derive(Debug, Clone)]
pub struct InferenceResponse {
    /// Provider content blocks (text and tool-use blocks).
    pub content: Vec<ContentBlock>,
    /// Provider stop reason label.
    pub stop_reason: String,
    /// Token accounting for this response.
    pub usage: Usage,
}

/// Token usage information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Prompt/input token count.
    pub input_tokens: u32,
    /// Generated/output token count.
    pub output_tokens: u32,
}

/// Port: perform LLM inference.
///
/// Implementations decide the backend (Anthropic, OpenAI, local Ollama, etc.).
#[async_trait::async_trait]
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

    /// Stream typed inference updates and one terminal structured response.
    ///
    /// Default implementation calls `infer()` and yields the full text as a
    /// typed deltas followed by the response, so all providers work without modification. Override in
    /// providers that have native SSE/streaming APIs.
    async fn infer_stream(&self, req: &InferenceRequest) -> InferStream {
        use futures::stream;

        match self.infer(req).await {
            Ok(response) => {
                let mut events = response
                    .content
                    .iter()
                    .enumerate()
                    .filter_map(|(index, block)| match block {
                        ContentBlock::Text { text } => Some(InferenceStreamEvent::Delta(
                            InferenceDelta::Text(text.clone()),
                        )),
                        ContentBlock::ToolUse { id, name, input } => {
                            Some(InferenceStreamEvent::Delta(InferenceDelta::ToolCall {
                                index,
                                id: Some(id.to_string()),
                                name: Some(name.to_string()),
                                arguments_fragment: input.to_string(),
                            }))
                        }
                        ContentBlock::ToolResult { .. } => None,
                    })
                    .map(Ok)
                    .collect::<Vec<_>>();
                events.push(Ok(InferenceStreamEvent::Final(response)));
                Box::pin(stream::iter(events))
            }
            Err(e) => Box::pin(stream::once(async move { Err(e) })),
        }
    }
}
