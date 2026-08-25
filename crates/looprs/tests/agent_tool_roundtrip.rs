use std::sync::{Arc, Mutex};

use looprs::providers::{InferenceRequest, InferenceResponse, LLMProvider, Usage};
use looprs::{Agent, ModelId, ToolId, ToolName};
use looprs_core::api::ContentBlock;

struct ScriptedProvider {
    model: ModelId,
    calls: Arc<Mutex<Vec<InferenceRequest>>>,
}

impl ScriptedProvider {
    fn new(calls: Arc<Mutex<Vec<InferenceRequest>>>) -> Self {
        Self {
            model: ModelId::new("test-model"),
            calls,
        }
    }
}

#[async_trait::async_trait]
impl LLMProvider for ScriptedProvider {
    async fn infer(
        &self,
        req: &InferenceRequest,
    ) -> Result<InferenceResponse, Box<dyn std::error::Error + Send + Sync>> {
        let mut calls = self.calls.lock().expect("calls lock must be available");
        calls.push(req.clone());
        let turn = calls.len();
        drop(calls);

        if turn == 1 {
            return Ok(InferenceResponse {
                content: vec![ContentBlock::ToolUse {
                    id: ToolId::new("tu-1"),
                    name: ToolName::new("bash"),
                    input: serde_json::json!({"command": "printf hi"}),
                }],
                stop_reason: "tool_use".to_string(),
                usage: Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                },
            });
        }

        Ok(InferenceResponse {
            content: vec![ContentBlock::Text {
                text: "done".to_string(),
            }],
            stop_reason: "end_turn".to_string(),
            usage: Usage {
                input_tokens: 11,
                output_tokens: 4,
            },
        })
    }

    fn name(&self) -> &str {
        "scripted"
    }

    fn model(&self) -> &ModelId {
        &self.model
    }

    fn validate_config(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

#[tokio::test]
async fn run_turn_executes_tool_and_requeries_provider() {
    let calls = Arc::new(Mutex::new(Vec::<InferenceRequest>::new()));
    let provider = ScriptedProvider::new(calls.clone());
    let mut agent = Agent::new(Box::new(provider)).expect("agent should initialize");

    agent.add_user_message("say hi");
    agent.run_turn().await.expect("turn should succeed");

    let calls = calls.lock().expect("calls lock must be available");
    assert_eq!(calls.len(), 2, "tool-use turn must requery provider once");

    let second = &calls[1].messages;
    let has_tool_result = second.iter().any(|msg| {
        msg.content
            .iter()
            .any(|b| matches!(b, ContentBlock::ToolResult { .. }))
    });
    assert!(
        has_tool_result,
        "second provider request must include tool_result content"
    );
}
