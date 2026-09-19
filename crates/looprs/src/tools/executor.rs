use serde_json::Value;

use crate::tools::{ToolContext, ToolError, execute_tool};

/// Port: dispatch a named agent tool call.
///
/// Abstracts the free `execute_tool` function so the Agent can be tested
/// with a stub executor instead of a real subprocess/filesystem backend.
#[async_trait::async_trait]
pub trait ToolExecutor: Send + Sync {
    async fn execute(
        &self,
        name: &str,
        args: &Value,
        ctx: &ToolContext,
    ) -> Result<String, ToolError>;
}

/// Production adapter: delegates to `tools::execute_tool`.
pub struct DefaultToolExecutor;

#[async_trait::async_trait]
impl ToolExecutor for DefaultToolExecutor {
    async fn execute(
        &self,
        name: &str,
        args: &Value,
        ctx: &ToolContext,
    ) -> Result<String, ToolError> {
        let name = name.to_string();
        let args = args.clone();
        let ctx = ctx.clone();
        tokio::task::spawn_blocking(move || execute_tool(&name, &args, &ctx))
            .await
            .map_err(|error| ToolError::CommandFailed(error.to_string()))?
    }
}

/// Test stub: always succeeds with a fixed response.
#[cfg(test)]
pub struct StubToolExecutor {
    pub response: String,
}

#[cfg(test)]
impl Default for StubToolExecutor {
    fn default() -> Self {
        Self {
            response: "ok".to_string(),
        }
    }
}

#[cfg(test)]
#[async_trait::async_trait]
impl ToolExecutor for StubToolExecutor {
    async fn execute(
        &self,
        _name: &str,
        _args: &Value,
        _ctx: &ToolContext,
    ) -> Result<String, ToolError> {
        Ok(self.response.clone())
    }
}

/// Conformance suite for the [`ToolExecutor`] port.
///
/// Contract:
/// 1. An unknown tool name returns `Err(ToolError::UnknownTool)` — never
///    `Ok`, and never a different error kind.
/// 2. Calling with arbitrary JSON args must not panic.
///
/// Call from each impl's `#[cfg(test)]` module.
#[cfg(test)]
pub async fn assert_tool_executor_contract(executor: &dyn ToolExecutor, ctx: &ToolContext) {
    let result = executor
        .execute(
            "definitely_not_a_real_tool_xyz",
            &serde_json::json!({}),
            ctx,
        )
        .await;
    match result {
        Err(ToolError::UnknownTool(name)) => {
            assert_eq!(name, "definitely_not_a_real_tool_xyz");
        }
        other => panic!("unknown tool must yield Err(ToolError::UnknownTool), got {other:?}"),
    }

    let _ = executor
        .execute("read", &serde_json::json!({"weird": [1, 2, 3]}), ctx)
        .await;
}

#[cfg(test)]
mod executor_tests {
    use super::*;
    use crate::fs_mode::FsMode;

    fn test_ctx() -> ToolContext {
        ToolContext::from_working_dir(std::env::current_dir().unwrap(), FsMode::Write)
    }

    #[tokio::test]
    async fn default_executor_satisfies_contract() {
        assert_tool_executor_contract(&DefaultToolExecutor, &test_ctx()).await;
    }

    /// StubToolExecutor is a canned-response double, not a dispatcher: it
    /// deliberately does NOT satisfy clause 1 of the port contract (unknown
    /// tool → UnknownTool), so only its own documented behaviour is asserted.
    #[tokio::test]
    async fn stub_executor_returns_response_verbatim() {
        let stub = StubToolExecutor {
            response: "canned".to_string(),
        };

        let out = stub
            .execute("anything", &serde_json::json!({"x": 1}), &test_ctx())
            .await
            .expect("stub must always succeed");
        assert_eq!(out, "canned");
    }
}
