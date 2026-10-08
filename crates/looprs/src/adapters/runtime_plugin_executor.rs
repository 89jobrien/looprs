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

// Discover an MCP server's tools, whether or not we're already inside a
// tokio runtime.
//
// Bootstrap (`load_extensions`, called from `#[tokio::main] async fn
// main()`) and `Agent::run_turn`'s synchronous `.execute()` dispatch both
// run this on a thread that is *already* driving the tokio runtime. Calling
// `Handle::block_on` directly from such a thread panics ("Cannot start a
// runtime from within a runtime") — it's not safe to reuse the handle the
// way a naive `Handle::try_current().unwrap_or_else(Runtime::new)` fallback
// would. `tokio::task::block_in_place` moves the blocking work off the
// async worker thread so blocking is actually safe; it requires a
// multi-thread runtime, which this workspace always uses (`rt-multi-thread`
// is enabled workspace-wide and `#[tokio::main]` defaults to that flavor).
//
// When there is no current runtime (e.g. a plain `#[test]`), spin up a
// throwaway one and block on that instead — `block_in_place` would panic
// outside of a runtime context.
fn block_on_discovery(server_url: &str) -> anyhow::Result<Vec<ToolDefinition>> {
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => tokio::task::block_in_place(|| {
            handle.block_on(crate::tools::mcp_tool_definitions(server_url))
        }),
        Err(_) => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(crate::tools::mcp_tool_definitions(server_url))
        }
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
        let builtin_names: std::collections::HashSet<String> = crate::tools::get_tool_definitions()
            .into_iter()
            .map(|d| d.name)
            .collect();

        let mut definitions: Vec<ToolDefinition> = Vec::new();
        let mut routes: HashMap<String, String> = HashMap::new();

        for manifest in runtime_plugins
            .into_iter()
            .filter(|m| m.enabled && m.entry.is_some())
        {
            // Filtered to `Some(entry)` above; skip defensively rather than
            // `.expect()` on an invariant that lives one line away from
            // where it's upheld.
            let Some(entry) = manifest.entry.as_ref() else {
                continue;
            };
            let server_url = entry.command.clone();

            match block_on_discovery(&server_url) {
                Ok(remote_defs) => {
                    let before: std::collections::HashSet<String> =
                        definitions.iter().map(|d| d.name.clone()).collect();
                    let (safe_defs, rejected): (Vec<_>, Vec<_>) = remote_defs
                        .into_iter()
                        .partition(|d| !builtin_names.contains(&d.name));
                    for d in &rejected {
                        log::warn!(
                            "rejecting Runtime-kind manifest '{}' tool '{}': collides with a built-in tool name and would otherwise intercept dispatch under that identity",
                            manifest.name,
                            d.name
                        );
                    }
                    definitions = merge_tool_definitions(definitions, safe_defs);
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

        if ctx.fs_mode() != crate::fs_mode::FsMode::Write {
            return Err(ToolError::ModeDenied {
                tool: name.to_string(),
                mode: ctx.fs_mode().as_str().to_string(),
                reason: "Runtime-kind manifest tools may have write side effects".to_string(),
            });
        }

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

    // Regression: PR #65/#64 review finding — an MCP server could advertise
    // a tool whose name collides with a built-in (e.g. "bash"). The
    // model-facing definitions list already filtered this via
    // `merge_tool_definitions`, but the `routes` table built alongside it
    // did not apply the same filter, so the colliding name would silently
    // dispatch to the remote MCP server instead of falling through to the
    // real built-in — untrusted plugin code executing under a trusted
    // built-in tool's identity, invisible to the model. This pins that a
    // discovered tool name colliding with a built-in never enters `routes`
    // or `definitions`, and dispatch for that name falls through to `inner`.
    #[test]
    fn discovered_tool_colliding_with_builtin_name_is_rejected_not_routed() {
        let builtin_names: Vec<String> = crate::tools::get_tool_definitions()
            .into_iter()
            .map(|d| d.name)
            .collect();
        assert!(
            builtin_names.iter().any(|n| n == "bash"),
            "expected 'bash' to be a real built-in tool name; got {builtin_names:?}"
        );

        let url = spawn_stub_mcp_server("bash");
        let bridge = ManifestRuntimeBridge::new(
            Box::new(StubToolExecutor {
                response: "from real builtin via inner".to_string(),
            }),
            vec![runtime_manifest("sidecar", &url)],
        );

        // Never exposed to the model under the built-in's name.
        assert!(
            bridge.tool_definitions().is_empty(),
            "a discovered tool colliding with a built-in name must not produce a tool definition"
        );

        // Never routed to the MCP server either — dispatch must fall
        // through to `inner` exactly like any other unmatched name.
        let output = bridge
            .execute("bash", &serde_json::json!({}), &test_ctx())
            .expect("fallback executor should handle the collided name");
        assert_eq!(output, "from real builtin via inner");
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

    // Regression: issue #64 follow-up — `block_on_discovery` (and the
    // identical pattern in `mcp_executor.rs::try_mcp`) used to call
    // `Handle::try_current().unwrap_or_else(Runtime::new)` and then
    // `.block_on(...)` directly on whichever handle it got, including a
    // handle for the runtime *currently driving the calling thread*. That's
    // exactly the real call path: `load_extensions()` runs inside
    // `#[tokio::main] async fn main()`, and `Agent::run_turn`'s sync
    // `.execute()` call fires on every tool dispatch, also from within the
    // runtime. `Handle::block_on` called from a thread that thread is
    // itself using to drive that same runtime panics ("Cannot start a
    // runtime from within a runtime"). A plain `#[test]` can't exercise
    // this: `Handle::try_current()` always fails outside a runtime, so
    // every existing test above only ever hits the "spin up a throwaway
    // Runtime" branch, not the handle-reuse branch where the bug lived.
    // This test runs `ManifestRuntimeBridge::new` (discovery) and
    // `execute` (dispatch) from inside an active multi-thread runtime,
    // mirroring both real call sites, and would have panicked before the
    // `block_in_place` fix.
    #[tokio::test(flavor = "multi_thread")]
    async fn discovery_and_dispatch_do_not_panic_when_called_from_within_a_runtime() {
        let url = spawn_stub_mcp_server("remote-search");

        // Constructed synchronously on this runtime-driven thread, exactly
        // like `load_extensions()` does inside `#[tokio::main]`.
        let bridge = ManifestRuntimeBridge::new(
            Box::new(StubToolExecutor {
                response: "fallback should not run".to_string(),
            }),
            vec![runtime_manifest("sidecar", &url)],
        );
        assert_eq!(bridge.tool_definitions().len(), 1);

        // Dispatched synchronously too, exactly like `Agent::run_turn`'s
        // `.execute()` call on every tool invocation.
        let output = bridge
            .execute("remote-search", &serde_json::json!({}), &test_ctx())
            .expect("mcp-backed tool should execute successfully from within a runtime");
        assert_eq!(output, "dispatched");
    }
}
