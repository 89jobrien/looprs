//! RetryProvider adapter — wraps any `LLMProvider` with exponential backoff.
//!
//! Retries up to `MAX_ATTEMPTS` times with a base delay of 500 ms, doubling on
//! each attempt.  If all attempts fail the last error is returned.
//!
//! | Attempt | Delay before retry |
//! |---------|-------------------|
//! | 1 (initial) | — |
//! | 2 | 500 ms |
//! | 3 | 1 000 ms |

use std::time::Duration;

use async_trait::async_trait;
use tokio::time::sleep;

use crate::providers::{InferenceRequest, InferenceResponse, LLMProvider};
use crate::types::ModelId;

const MAX_ATTEMPTS: u32 = 3;
const BASE_DELAY_MS: u64 = 500;

/// Wraps an inner `LLMProvider` and retries failed `infer` calls with
/// exponential backoff.
///
/// All other `LLMProvider` methods (`name`, `model`, `validate_config`,
/// `supports_tool_use`) are delegated to the inner provider unchanged.
pub struct RetryProvider<P: LLMProvider> {
    inner: P,
    max_attempts: u32,
    base_delay_ms: u64,
}

impl<P: LLMProvider> RetryProvider<P> {
    /// Wrap `inner` with the default retry policy (3 attempts, 500 ms base).
    pub fn new(inner: P) -> Self {
        Self {
            inner,
            max_attempts: MAX_ATTEMPTS,
            base_delay_ms: BASE_DELAY_MS,
        }
    }

    /// Override the base delay in milliseconds.
    pub fn with_base_delay_ms(mut self, ms: u64) -> Self {
        self.base_delay_ms = ms;
        self
    }
}

#[async_trait]
impl<P: LLMProvider> LLMProvider for RetryProvider<P> {
    async fn infer(
        &self,
        req: &InferenceRequest,
    ) -> Result<InferenceResponse, Box<dyn std::error::Error + Send + Sync>> {
        let mut last_err: Option<Box<dyn std::error::Error + Send + Sync>> = None;

        for attempt in 0..self.max_attempts {
            if attempt > 0 {
                let delay_ms = self.base_delay_ms * (1u64 << (attempt - 1));
                sleep(Duration::from_millis(delay_ms)).await;
            }

            match self.inner.infer(req).await {
                Ok(resp) => return Ok(resp),
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }

        Err(last_err.expect("max_attempts >= 1, so at least one attempt ran"))
    }

    fn name(&self) -> &str {
        self.inner.name()
    }

    fn model(&self) -> &ModelId {
        self.inner.model()
    }

    fn validate_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.inner.validate_config()
    }

    fn supports_tool_use(&self) -> bool {
        self.inner.supports_tool_use()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    use crate::api::{ContentBlock, Message as ApiMessage};
    use crate::errors::ProviderError;
    use crate::providers::Usage;

    struct CountingProvider {
        calls: Arc<AtomicU32>,
        fail_times: u32,
        model: ModelId,
    }

    impl CountingProvider {
        fn new(fail_times: u32) -> Self {
            Self {
                calls: Arc::new(AtomicU32::new(0)),
                fail_times,
                model: ModelId::new("test-model"),
            }
        }
        fn call_count(&self) -> u32 {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl LLMProvider for CountingProvider {
        async fn infer(
            &self,
            _req: &InferenceRequest,
        ) -> Result<InferenceResponse, Box<dyn std::error::Error + Send + Sync>> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if n <= self.fail_times {
                Err(ProviderError::ApiError(format!("simulated failure #{n}")).into())
            } else {
                Ok(InferenceResponse {
                    content: vec![ContentBlock::Text {
                        text: "ok".to_string(),
                    }],
                    stop_reason: "end_turn".to_string(),
                    usage: Usage {
                        input_tokens: 1,
                        output_tokens: 1,
                    },
                })
            }
        }

        fn name(&self) -> &str {
            "counting"
        }
        fn model(&self) -> &ModelId {
            &self.model
        }
        fn validate_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
    }

    fn dummy_req() -> InferenceRequest {
        InferenceRequest {
            model: ModelId::new("test-model"),
            messages: vec![ApiMessage::user("hi")],
            tools: vec![],
            max_tokens: 16,
            temperature: None,
            system: String::new(),
        }
    }

    #[tokio::test]
    async fn succeeds_on_first_try() {
        let inner = CountingProvider::new(0);
        let provider = RetryProvider::new(inner).with_base_delay_ms(0);
        let result = provider.infer(&dummy_req()).await;
        assert!(result.is_ok());
        assert_eq!(provider.inner.call_count(), 1);
    }

    #[tokio::test]
    async fn retries_and_succeeds() {
        let inner = CountingProvider::new(2); // fail twice, succeed on 3rd
        let provider = RetryProvider::new(inner).with_base_delay_ms(0);
        let result = provider.infer(&dummy_req()).await;
        assert!(result.is_ok());
        assert_eq!(provider.inner.call_count(), 3);
    }

    #[tokio::test]
    async fn exhausts_retries_returns_last_error() {
        let inner = CountingProvider::new(5); // always fails within 3 attempts
        let provider = RetryProvider::new(inner).with_base_delay_ms(0);
        let result = provider.infer(&dummy_req()).await;
        assert!(result.is_err());
        assert_eq!(provider.inner.call_count(), 3);
        let err = result.unwrap_err().to_string();
        assert!(err.contains("simulated failure #3"), "got: {err}");
    }

    #[tokio::test]
    async fn delegates_metadata() {
        let inner = CountingProvider::new(0);
        let provider = RetryProvider::new(inner);
        assert_eq!(provider.name(), "counting");
        assert_eq!(provider.model().as_str(), "test-model");
        assert!(provider.validate_config().is_ok());
    }
}
