use serde_json::Value;

use crate::tools::{ToolContext, ToolError, ToolExecutor};

/// Adapter: routes tool calls to a remote MCP server via HTTP/JSON-RPC.
///
/// Implements the async `ToolExecutor` port. Each `execute()` call posts a
/// `tools/call` JSON-RPC request to `server_url` and returns the text result.
///
/// Use this when you want the agent to dispatch tool calls to an external
/// MCP server instead of (or alongside) built-in tools. For built-in tools
/// keep the default `DefaultToolExecutor`.
pub struct McpToolExecutor {
    server_url: String,
    /// Fallback executor for tools not found on the MCP server.
    fallback: Option<Box<dyn ToolExecutor>>,
}

impl McpToolExecutor {
    /// Route all tool calls to `server_url`. No fallback.
    pub fn new(server_url: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
            fallback: None,
        }
    }

    /// Try the MCP server first; fall back to `fallback` on any error.
    pub fn with_fallback(server_url: impl Into<String>, fallback: Box<dyn ToolExecutor>) -> Self {
        Self {
            server_url: server_url.into(),
            fallback: Some(fallback),
        }
    }

    async fn try_mcp(&self, name: &str, args: &Value) -> Result<String, anyhow::Error> {
        crate::tools::mcp_tool_call(&self.server_url, name, args.clone()).await
    }
}

#[async_trait::async_trait]
impl ToolExecutor for McpToolExecutor {
    async fn execute(
        &self,
        name: &str,
        args: &Value,
        ctx: &ToolContext,
    ) -> Result<String, ToolError> {
        match self.try_mcp(name, args).await {
            Ok(output) => Ok(output),
            Err(e) => {
                if let Some(ref fb) = self.fallback {
                    fb.execute(name, args, ctx).await
                } else {
                    Err(ToolError::CommandFailed(e.to_string()))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::executor::StubToolExecutor;

    #[tokio::test]
    async fn mcp_executor_falls_back_on_error() {
        // Server URL that will always fail (no server running)
        let stub = StubToolExecutor {
            response: "fallback-result".to_string(),
        };
        let executor = McpToolExecutor::with_fallback(
            "http://127.0.0.1:0/mcp", // port 0 → immediate connection refused
            Box::new(stub),
        );

        let ctx = ToolContext::from_working_dir(
            std::env::current_dir().unwrap(),
            crate::fs_mode::FsMode::Write,
        );
        let result = executor
            .execute("echo", &serde_json::json!({"text": "hi"}), &ctx)
            .await;
        assert_eq!(result.unwrap(), "fallback-result");
    }

    #[tokio::test]
    async fn mcp_executor_errors_without_fallback() {
        let executor = McpToolExecutor::new("http://127.0.0.1:0/mcp");
        let ctx = ToolContext::from_working_dir(
            std::env::current_dir().unwrap(),
            crate::fs_mode::FsMode::Write,
        );
        let result = executor.execute("echo", &serde_json::json!({}), &ctx).await;
        assert!(result.is_err());
    }

    /// With a DefaultToolExecutor fallback, the adapter satisfies the port
    /// contract: unknown tools surface as UnknownTool from the fallback.
    #[tokio::test]
    async fn mcp_executor_with_default_fallback_satisfies_contract() {
        use crate::tools::executor::{DefaultToolExecutor, assert_tool_executor_contract};

        let executor =
            McpToolExecutor::with_fallback("http://127.0.0.1:0/mcp", Box::new(DefaultToolExecutor));
        let ctx = ToolContext::from_working_dir(
            std::env::current_dir().unwrap(),
            crate::fs_mode::FsMode::Write,
        );
        assert_tool_executor_contract(&executor, &ctx).await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn mcp_executor_is_safe_inside_tokio_runtime() {
        let stub = StubToolExecutor {
            response: "fallback-result".to_string(),
        };
        let executor = McpToolExecutor::with_fallback("http://127.0.0.1:0/mcp", Box::new(stub));
        let ctx = ToolContext::from_working_dir(
            std::env::current_dir().unwrap(),
            crate::fs_mode::FsMode::Write,
        );

        let result = executor
            .execute("echo", &serde_json::json!({"text": "hi"}), &ctx)
            .await;

        assert_eq!(result.unwrap(), "fallback-result");
    }
}
