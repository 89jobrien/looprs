//! Adapts MCP discovery and execution into the runtime's tool catalog and dispatcher ports.

use serde_json::Value;

use std::collections::HashSet;
use std::sync::Arc;

use crate::api::ToolDefinition;
use crate::tools::{ToolCatalog, ToolContext, ToolError, ToolExecutor};

/// Adapter: combines a local catalog with MCP discovery.
pub struct McpToolCatalog {
    server_url: String,
    local: Arc<dyn ToolCatalog>,
}

impl std::fmt::Debug for McpToolCatalog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("McpToolCatalog")
            .field("server_url", &self.server_url)
            .finish_non_exhaustive()
    }
}

impl McpToolCatalog {
    /// Create an MCP catalog that preserves `local` definitions on conflicts.
    pub fn new(server_url: impl Into<String>, local: Arc<dyn ToolCatalog>) -> Self {
        Self {
            server_url: server_url.into(),
            local,
        }
    }
}

#[async_trait::async_trait]
impl ToolCatalog for McpToolCatalog {
    async fn definitions(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        let local = self.local.definitions().await?;
        match crate::tools::mcp_tool_definitions(&self.server_url).await {
            Ok(remote) => Ok(merge_tool_definitions(local, remote)),
            Err(error) => {
                log::warn!(
                    "failed to discover MCP tools from {}: {error}",
                    self.server_url
                );
                Ok(local)
            }
        }
    }
}

fn merge_tool_definitions(
    mut local: Vec<ToolDefinition>,
    remote: Vec<ToolDefinition>,
) -> Vec<ToolDefinition> {
    // TODO(feature-idea 12): Make tool registration origin-aware and collision-safe. (#59)
    // Reject or namespace schema/dispatcher collisions.
    let mut known = local
        .iter()
        .map(|tool| tool.name.clone())
        .collect::<HashSet<_>>();
    local.extend(
        remote
            .into_iter()
            .filter(|tool| known.insert(tool.name.clone())),
    );
    local
}

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

    fn definition(name: &str, description: &str) -> ToolDefinition {
        ToolDefinition {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: serde_json::json!({"type": "object"}),
        }
    }

    #[test]
    fn merging_empty_catalogs_is_empty() {
        assert!(merge_tool_definitions(Vec::new(), Vec::new()).is_empty());
    }

    #[test]
    fn local_catalog_takes_precedence_over_remote_duplicates() {
        let merged = merge_tool_definitions(
            vec![definition("read", "local")],
            vec![definition("read", "remote"), definition("remote", "remote")],
        );

        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].description, "local");
        assert_eq!(merged[1].name, "remote");
    }

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
