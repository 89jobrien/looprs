//! Bridges `PluginKind::Runtime` manifests (external MCP servers) into the
//! agent's tool-call loop.
//!
//! Mirrors [`crate::adapters::plugin_executor::ManifestToolExecutor`] (the
//! `PluginKind::Tool` bridge) but differs in two structural ways driven by
//! what a Runtime-kind manifest actually represents:
//!
//! - `PluginEntry.command` is interpreted as the already-running MCP server's
//!   URL, not a subprocess argv[0]. This is consistent with
//!   `PluginExecutionMode::Daemon` semantics: the server is expected to be
//!   externally supervised, not launched per call the way Tool-kind's OneShot
//!   subprocess is. See the doc comment on `PluginEntry` for the same note.
//! - One MCP server can expose an arbitrary number of differently-named
//!   tools, discovered only at `tools/list` time, so routing is keyed by
//!   discovered tool name -> owning server URL (`HashMap<String, String>`),
//!   not by manifest name the way `ManifestToolExecutor` does.

use std::collections::HashMap;

use crate::api::ToolDefinition;
use crate::plugins::manifests::PluginManifest;
use crate::tools::{ToolContext, ToolError, ToolExecutor, merge_tool_definitions};

use super::mcp_executor::McpToolExecutor;

// Small helper to avoid requiring a full tokio runtime when we're already
// inside one. Mirrors the identically-named helper in `mcp_executor.rs`;
// kept private/local rather than shared since both are tiny and
// implementation-detail only.
enum Either {
    Handle(tokio::runtime::Handle),
    Runtime(tokio::runtime::Runtime),
}

fn block_on_discovery(server_url: &str) -> anyhow::Result<Vec<ToolDefinition>> {
    let rt = tokio::runtime::Handle::try_current()
        .map(Either::Handle)
        .unwrap_or_else(|_| Either::Runtime(tokio::runtime::Runtime::new().unwrap()));

    match rt {
        Either::Handle(h) => h.block_on(crate::tools::mcp_tool_definitions(server_url)),
        Either::Runtime(rt) => rt.block_on(crate::tools::mcp_tool_definitions(server_url)),
    }
}

/// Bridges `PluginKind::Runtime` manifests into the agent's tool-call loop.
///
/// Wraps a fallback [`ToolExecutor`] (normally [`crate::tools::DefaultToolExecutor`],
/// or an already-constructed `ManifestToolExecutor` when Tool-kind manifests
/// are also enabled — see `extensions.rs::load_extensions` for the composed
/// chain). At construction time, every enabled Runtime-kind manifest with an
/// `entry` is treated as an MCP server URL and probed via `tools/list`
/// (`mcp_tool_definitions`). A server that is unreachable or returns an
/// error is skipped with a `log::warn!` — matching how `merge_tool_definitions`
/// degrades gracefully on a name collision rather than erroring — so one bad
/// MCP server can never abort CLI bootstrap.
///
/// Discovered tool definitions across all servers are merged via
/// `merge_tool_definitions`, first-registered-server wins on a name
/// collision. Routing is keyed by discovered tool name, not manifest name,
/// since a single server can expose many tools.
pub struct ManifestRuntimeBridge {
    inner: Box<dyn ToolExecutor>,
    /// Discovered tool name -> owning MCP server URL.
    routes: HashMap<String, String>,
    definitions: Vec<ToolDefinition>,
}

impl ManifestRuntimeBridge {
    /// `runtime_plugins` should come from `PluginRuntimeRegistry::list_runtime_plugins()`.
    /// Manifests that are disabled or carry no `entry` are dropped. Each
    /// remaining manifest's `entry.command` is treated as an MCP server URL
    /// and probed synchronously (blocking on a tokio runtime, reused if one
    /// is already active); a server that fails to respond is skipped with a
    /// warning rather than failing construction.
    pub fn new(inner: Box<dyn ToolExecutor>, runtime_plugins: Vec<PluginManifest>) -> Self {
        let mut definitions: Vec<ToolDefinition> = Vec::new();
        let mut routes: HashMap<String, String> = HashMap::new();

        for manifest in runtime_plugins
            .into_iter()
            .filter(|m| m.enabled && m.entry.is_some())
        {
            // Invariant: filtered to `Some(entry)` above.
            let server_url = manifest
                .entry
                .as_ref()
                .expect("ManifestRuntimeBridge only processes manifests with Some(entry)")
                .command
                .clone();

            match block_on_discovery(&server_url) {
                Ok(remote_defs) => {
                    let before: std::collections::HashSet<String> =
                        definitions.iter().map(|d| d.name.clone()).collect();
                    definitions = merge_tool_definitions(definitions, remote_defs);
                    for def in &definitions {
                        if !before.contains(&def.name) {
                            routes
                                .entry(def.name.clone())
                                .or_insert_with(|| server_url.clone());
                        }
                    }
                }
                Err(e) => {
                    log::warn!(
                        "skipping Runtime-kind manifest '{}': MCP server '{}' unreachable: {e}",
                        manifest.name,
                        server_url
                    );
                }
            }
        }

        Self {
            inner,
            routes,
            definitions,
        }
    }

    /// `ToolDefinition`s discovered from the MCP servers this bridge can
    /// dispatch to. Merge these into the request's tool list alongside the
    /// built-in set and any Tool-kind manifest definitions.
    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        self.definitions.clone()
    }
}

impl ToolExecutor for ManifestRuntimeBridge {
    fn execute(
        &self,
        name: &str,
        args: &serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<String, ToolError> {
        let Some(server_url) = self.routes.get(name) else {
            return self.inner.execute(name, args, ctx);
        };

        McpToolExecutor::new(server_url.clone()).execute(name, args, ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_mode::FsMode;
    use crate::plugins::manifests::PluginEntry;
    use crate::tools::executor::StubToolExecutor;
    use looprs_core::ports::{PluginExecutionMode, PluginKind};
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::thread;

    fn test_ctx() -> ToolContext {
        ToolContext::from_working_dir(std::env::current_dir().unwrap(), FsMode::Write)
    }

    fn runtime_manifest(name: &str, server_url: &str) -> PluginManifest {
        PluginManifest {
            name: name.to_string(),
            kind: PluginKind::Runtime,
            description: None,
            enabled: true,
            required: false,
            mode: PluginExecutionMode::default(),
            entry: Some(PluginEntry {
                command: server_url.to_string(),
            }),
            triggers: Vec::new(),
            route_to_agent: None,
        }
    }

    /// Minimal blocking stub MCP server: answers every POST with a
    /// `tools/list` response advertising `tool_name`, or (if the request
    /// body contains `"tools/call"`) a fixed `tools/call` text result.
    /// Mirrors `mcp_executor.rs`'s own preference for a tiny local listener
    /// over spinning up a real MCP implementation in tests.
    fn spawn_stub_mcp_server(tool_name: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        thread::spawn(move || {
            // Handle up to 2 requests: discovery (`tools/list`) and dispatch
            // (`tools/call`), each on its own connection (reqwest doesn't
            // reuse the connection across the two distinct calls here).
            for stream in listener.incoming().take(2) {
                let mut stream = match stream {
                    Ok(s) => s,
                    Err(_) => continue,
                };
                let mut reader = BufReader::new(stream.try_clone().unwrap());
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
                let body_str = String::from_utf8_lossy(&body);

                let json = if body_str.contains("tools/call") {
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "result": { "content": [{ "type": "text", "text": "dispatched" }] }
                    })
                } else {
                    serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "result": { "tools": [{ "name": tool_name, "description": "stub" }] }
                    })
                };
                let payload = json.to_string();
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

    // Regression: issue #64 — PluginKind::Runtime manifests had no bridge
    // into the agent's tool-call loop at all. These tests pin the actual
    // discovery/dispatch behavior, mirroring plugin_executor.rs's 4-case
    // pattern for the Tool-kind bridge.

    #[test]
    fn enabled_runtime_manifest_is_exposed_as_a_tool_definition() {
        let url = spawn_stub_mcp_server("remote-search");
        let bridge = ManifestRuntimeBridge::new(
            Box::new(StubToolExecutor::default()),
            vec![runtime_manifest("sidecar", &url)],
        );

        let definitions = bridge.tool_definitions();
        let names: Vec<&str> = definitions.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["remote-search"]);
    }

    #[test]
    fn disabled_or_entry_less_manifests_produce_no_tool_definitions() {
        let mut disabled = runtime_manifest("disabled-runtime", "http://127.0.0.1:0/mcp");
        disabled.enabled = false;
        let mut no_entry = runtime_manifest("no-entry-runtime", "http://127.0.0.1:0/mcp");
        no_entry.entry = None;

        let bridge = ManifestRuntimeBridge::new(
            Box::new(StubToolExecutor::default()),
            vec![disabled, no_entry],
        );

        assert!(bridge.tool_definitions().is_empty());
    }

    #[test]
    fn matching_tool_call_dispatches_to_the_mcp_server_not_the_fallback() {
        let url = spawn_stub_mcp_server("remote-search");
        let bridge = ManifestRuntimeBridge::new(
            Box::new(StubToolExecutor {
                response: "fallback should not run".to_string(),
            }),
            vec![runtime_manifest("sidecar", &url)],
        );

        let output = bridge
            .execute("remote-search", &serde_json::json!({}), &test_ctx())
            .expect("mcp-backed tool should execute successfully");
        assert_eq!(output, "dispatched");
    }

    #[test]
    fn unmatched_tool_call_falls_back_to_inner_executor() {
        let url = spawn_stub_mcp_server("remote-search");
        let bridge = ManifestRuntimeBridge::new(
            Box::new(StubToolExecutor {
                response: "from inner".to_string(),
            }),
            vec![runtime_manifest("sidecar", &url)],
        );

        let output = bridge
            .execute("not_a_runtime_tool", &serde_json::json!({}), &test_ctx())
            .expect("fallback executor should handle unmatched names");
        assert_eq!(output, "from inner");
    }

    #[test]
    fn unreachable_mcp_server_is_skipped_without_panicking() {
        // Port 0 → immediate connection refused, no stub server listening.
        let bridge = ManifestRuntimeBridge::new(
            Box::new(StubToolExecutor::default()),
            vec![runtime_manifest("unreachable", "http://127.0.0.1:0/mcp")],
        );

        assert!(bridge.tool_definitions().is_empty());
    }
}
