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
                let rt = tokio::runtime::Runtime::new()?;
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

    /// Minimal stand-in MCP server that answers any `tools/call` request
    /// with a fixed text result, on one connection, then exits. Mirrors
    /// `runtime_plugin_executor.rs::spawn_stub_mcp_server`'s `tools/call`
    /// branch but lives here so this module's block_in_place regression
    /// test doesn't need to reach into that module's private test helper.
    fn spawn_stub_mcp_call_server() -> String {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub mcp server");
        let addr = listener.local_addr().expect("local_addr");

        std::thread::spawn(move || {
            if let Ok((stream, _)) = listener.accept() {
                let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
                let mut stream = stream;

                let mut content_length = 0usize;
                let mut line = String::new();
                loop {
                    line.clear();
                    if reader.read_line(&mut line).unwrap_or(0) == 0 {
                        break;
                    }
                    if line == "\r\n" || line == "\n" {
                        break;
                    }
                    if let Some(len) = line
                        .to_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().to_string())
                    {
                        content_length = len.parse().unwrap_or(0);
                    }
                }
                let mut body = vec![0u8; content_length];
                std::io::Read::read_exact(&mut reader, &mut body).ok();

                let payload = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": { "content": [{ "type": "text", "text": "dispatched-from-stub" }] }
                })
                .to_string();
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    payload.len(),
                    payload
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });

        format!("http://{addr}/mcp")
    }

    // Regression: issue #64 follow-up (PR #65 review finding c) — the
    // `block_on_discovery`/`try_mcp` pattern of reusing
    // `Handle::try_current()` and calling `.block_on` directly panicked
    // ("Cannot start a runtime from within a runtime") when invoked from a
    // thread already driving the tokio runtime — exactly how
    // `Agent::run_turn`'s synchronous `.execute()` dispatch calls it. The
    // `runtime_plugin_executor.rs` suite covers `block_on_discovery` from
    // inside an active runtime; this test exercises `McpToolExecutor::
    // try_mcp`'s own identical `block_in_place` path directly, via a real
    // `tools/call` round trip against a stub MCP server, so a regression in
    // *this* call site specifically would be caught even if the
    // runtime-bridge test above it were ever removed or changed.
    #[tokio::test(flavor = "multi_thread")]
    async fn try_mcp_does_not_panic_when_called_from_within_a_runtime() {
        let url = spawn_stub_mcp_call_server();
        let executor = McpToolExecutor::new(url);
        let ctx = ToolContext::from_working_dir(
            std::env::current_dir().unwrap(),
            crate::fs_mode::FsMode::Write,
        );

        // Constructed and dispatched synchronously on this runtime-driven
        // thread — mirroring the real `Agent::run_turn` call path — to
        // actually exercise the `Handle::try_current().is_ok()` /
        // `block_in_place` branch rather than the throwaway-Runtime
        // fallback a plain `#[test]` would hit.
        let result = executor.execute("whatever", &serde_json::json!({}), &ctx);
        assert_eq!(
            result.expect("tools/call should succeed"),
            "dispatched-from-stub"
        );
    }

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
