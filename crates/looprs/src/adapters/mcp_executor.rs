use serde_json::Value;

use crate::tools::{ToolContext, ToolError, ToolExecutor};

/// Adapter: routes tool calls to a remote MCP server via HTTP/JSON-RPC.
///
/// Implements `ToolExecutor` so it can be injected into `Agent` via
/// `with_tool_executor()`. Each `execute()` call posts a `tools/call`
/// JSON-RPC request to `server_url` and returns the text result.
///
/// Use this when you want the agent to dispatch tool calls to an external
/// MCP server instead of (or alongside) built-in tools. For built-in tools
/// keep the default `DefaultToolExecutor`.
pub struct McpToolExecutor {
    server_url: String,
    /// Fallback executor for tools not found on the MCP server.
    fallback: Option<Box<dyn ToolExecutor>>,
}

// TODO(feature-idea-10): Compose MCP configuration and tool discovery into the
// production runtime so remote definitions are included in inference requests.
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

    fn try_mcp(&self, name: &str, args: &Value) -> Result<String, anyhow::Error> {
        let url = self.server_url.clone();
        let name = name.to_string();
        let args = args.clone();

        // See the identical pattern (and its rationale) in
        // `runtime_plugin_executor.rs::block_on_discovery`: a thread already
        // driving the tokio runtime (as both bootstrap and
        // `Agent::run_turn`'s sync dispatch path are) cannot safely
        // `Handle::block_on` directly — that panics. `block_in_place` is the
        // safe way to block from inside a multi-thread runtime; outside a
        // runtime, spin up a throwaway one instead.
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| {
                handle.block_on(crate::tools::mcp_tool_call(&url, &name, args))
            }),
            Err(_) => {
                let rt = tokio::runtime::Runtime::new().unwrap();
                rt.block_on(crate::tools::mcp_tool_call(&url, &name, args))
            }
        }
    }
}

impl ToolExecutor for McpToolExecutor {
    fn execute(&self, name: &str, args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
        match self.try_mcp(name, args) {
            Ok(output) => Ok(output),
            Err(e) => {
                if let Some(ref fb) = self.fallback {
                    fb.execute(name, args, ctx)
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

    #[test]
    fn mcp_executor_falls_back_on_error() {
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
        let result = executor.execute("echo", &serde_json::json!({"text": "hi"}), &ctx);
        assert_eq!(result.unwrap(), "fallback-result");
    }

    #[test]
    fn mcp_executor_errors_without_fallback() {
        let executor = McpToolExecutor::new("http://127.0.0.1:0/mcp");
        let ctx = ToolContext::from_working_dir(
            std::env::current_dir().unwrap(),
            crate::fs_mode::FsMode::Write,
        );
        let result = executor.execute("echo", &serde_json::json!({}), &ctx);
        assert!(result.is_err());
    }

    /// With a DefaultToolExecutor fallback, the adapter satisfies the port
    /// contract: unknown tools surface as UnknownTool from the fallback.
    #[test]
    fn mcp_executor_with_default_fallback_satisfies_contract() {
        use crate::tools::executor::{DefaultToolExecutor, assert_tool_executor_contract};

        let executor =
            McpToolExecutor::with_fallback("http://127.0.0.1:0/mcp", Box::new(DefaultToolExecutor));
        let ctx = ToolContext::from_working_dir(
            std::env::current_dir().unwrap(),
            crate::fs_mode::FsMode::Write,
        );
        assert_tool_executor_contract(&executor, &ctx);
    }
}
