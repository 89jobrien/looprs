//! PluginExecutor port adapter — bridges domain to Plugins infrastructure.
//!
//! This adapter implements the PluginExecutor port using the existing Plugins
//! system (Runner + ToolRegistry). It provides a domain-facing interface
//! without exposing low-level subprocess details.

use std::collections::HashMap;
use std::ffi::OsString;
use std::process::Output;

use crate::api::ToolDefinition;
use crate::plugins::Plugins;
use crate::plugins::manifests::PluginManifest;
use crate::ports::PluginExecutor;
use crate::tools::{ToolContext, ToolError, ToolExecutor};

/// Adapter implementing the PluginExecutor port via the Plugins system.
///
/// Wraps a reference to Plugins to provide a domain-facing interface.
pub struct PluginsAdapter<'a> {
    plugins: &'a Plugins,
}

impl<'a> PluginsAdapter<'a> {
    /// Create a new adapter wrapping a Plugins reference.
    pub fn new(plugins: &'a Plugins) -> Self {
        Self { plugins }
    }

    /// Create an adapter using the system-wide Plugins singleton.
    pub fn system() -> Self {
        Self {
            plugins: Plugins::system(),
        }
    }
}

impl<'a> PluginExecutor for PluginsAdapter<'a> {
    fn has_tool(&self, tool: &str) -> bool {
        self.plugins.has_in_path(tool)
    }

    fn execute_tool(&self, tool: &str, args: Vec<OsString>) -> std::io::Result<Output> {
        self.plugins.output(tool, args)
    }

    fn execute_tool_if_available(&self, tool: &str, args: Vec<OsString>) -> Option<Output> {
        self.plugins.output_if_available(tool, args)
    }

    fn probe_tool_success(&self, tool: &str, args: Vec<OsString>) -> bool {
        self.plugins.probe_success(tool, args)
    }
}

/// Bridges `PluginKind::Tool` manifests into the agent's tool-call loop.
///
/// Wraps a fallback [`ToolExecutor`] (normally [`crate::tools::DefaultToolExecutor`])
/// and a [`PluginExecutor`] adapter. Calls whose name matches an enabled
/// Tool-kind manifest run that manifest's `entry.command` as a subprocess via
/// the `PluginExecutor` port; every other call name is delegated to `inner`
/// unchanged. Pair with [`ManifestToolExecutor::tool_definitions`], merged
/// into the request via `Agent::with_extra_tool_definitions`, so the model
/// actually sees these tools as callable.
pub struct ManifestToolExecutor {
    inner: Box<dyn ToolExecutor>,
    executor: Box<dyn PluginExecutor>,
    manifests: HashMap<String, PluginManifest>,
}

impl ManifestToolExecutor {
    /// `tool_plugins` should come from `PluginRuntimeRegistry::list_tool_plugins()`.
    /// Manifests that are disabled or carry no `entry` are dropped — they have
    /// nothing to execute.
    pub fn new(
        inner: Box<dyn ToolExecutor>,
        executor: Box<dyn PluginExecutor>,
        tool_plugins: Vec<PluginManifest>,
    ) -> Self {
        let manifests = tool_plugins
            .into_iter()
            .filter(|m| m.enabled && m.entry.is_some())
            .map(|m| (m.name.clone(), m))
            .collect();
        Self {
            inner,
            executor,
            manifests,
        }
    }

    /// `ToolDefinition`s for the manifest-backed tools this executor can run.
    /// Merge these into the request's tool list alongside the built-in set.
    pub fn tool_definitions(&self) -> Vec<ToolDefinition> {
        self.manifests
            .values()
            .map(|manifest| ToolDefinition {
                name: manifest.name.clone(),
                description: manifest
                    .description
                    .clone()
                    .unwrap_or_else(|| format!("Manifest-declared tool plugin: {}", manifest.name)),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "args": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Positional arguments passed to the plugin's command"
                        }
                    }
                }),
            })
            .collect()
    }
}

impl ToolExecutor for ManifestToolExecutor {
    fn execute(
        &self,
        name: &str,
        args: &serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<String, ToolError> {
        let Some(manifest) = self.manifests.get(name) else {
            return self.inner.execute(name, args, ctx);
        };

        if ctx.fs_mode() != crate::fs_mode::FsMode::Write {
            return Err(ToolError::ModeDenied {
                tool: name.to_string(),
                mode: ctx.fs_mode().as_str().to_string(),
                reason: "Tool-kind manifest plugins may have write side effects".to_string(),
            });
        }

        // `manifests` only holds entries filtered to `Some(entry)` in `new`;
        // skip defensively rather than `.expect()` on that invariant.
        let Some(entry) = manifest.entry.as_ref() else {
            return Err(ToolError::CommandFailed(format!(
                "plugin '{name}' has no entry command configured"
            )));
        };

        let extra_args: Vec<OsString> = args
            .get("args")
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(serde_json::Value::as_str)
                    .map(OsString::from)
                    .collect()
            })
            .unwrap_or_default();

        let output = self
            .executor
            .execute_tool(&entry.command, extra_args)
            .map_err(ToolError::Io)?;

        if !output.status.success() {
            return Err(ToolError::CommandFailed(format!(
                "plugin '{name}' exited with status {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs_mode::FsMode;
    use crate::plugins::manifests::PluginEntry;
    use crate::tools::executor::StubToolExecutor;
    use looprs_core::ports::{PluginExecutionMode, PluginKind};

    #[test]
    fn adapts_has_tool_to_has_in_path() {
        let adapter = PluginsAdapter::system();
        // /bin/echo should exist on most unix systems
        assert!(adapter.has_tool("echo"));
        assert!(!adapter.has_tool("nonexistent_tool_xyz_12345"));
    }

    #[test]
    fn adapts_probe_tool_success() {
        let adapter = PluginsAdapter::system();
        // true is a builtin in most shells, but we test with a standard tool
        assert!(adapter.probe_tool_success("echo", vec![]));
    }

    fn test_ctx() -> ToolContext {
        ToolContext::from_working_dir(std::env::current_dir().unwrap(), FsMode::Write)
    }

    fn tool_manifest(name: &str, command: &str) -> PluginManifest {
        PluginManifest {
            name: name.to_string(),
            kind: PluginKind::Tool,
            description: None,
            enabled: true,
            required: false,
            mode: PluginExecutionMode::default(),
            entry: Some(PluginEntry {
                command: command.to_string(),
            }),
            triggers: Vec::new(),
            route_to_agent: None,
        }
    }

    // Regression: issue #58 finding 3 — PluginKind::Tool manifests were parsed
    // and supervised but had no bridge into the agent's tool-call loop at all;
    // the executor existed in name only. These tests pin the actual dispatch.

    #[test]
    fn enabled_tool_manifest_is_exposed_as_a_tool_definition() {
        let executor = ManifestToolExecutor::new(
            Box::new(StubToolExecutor::default()),
            Box::new(PluginsAdapter::system()),
            vec![tool_manifest("formatter", "echo")],
        );

        let definitions = executor.tool_definitions();
        let names: Vec<&str> = definitions.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, vec!["formatter"]);
    }

    #[test]
    fn disabled_or_entry_less_manifests_produce_no_tool_definitions() {
        let mut disabled = tool_manifest("disabled-tool", "echo");
        disabled.enabled = false;
        let mut no_entry = tool_manifest("no-entry-tool", "echo");
        no_entry.entry = None;

        let executor = ManifestToolExecutor::new(
            Box::new(StubToolExecutor::default()),
            Box::new(PluginsAdapter::system()),
            vec![disabled, no_entry],
        );

        assert!(executor.tool_definitions().is_empty());
    }

    #[test]
    fn matching_tool_call_runs_the_manifest_command_not_the_fallback() {
        let executor = ManifestToolExecutor::new(
            Box::new(StubToolExecutor {
                response: "fallback should not run".to_string(),
            }),
            Box::new(PluginsAdapter::system()),
            vec![tool_manifest("echoer", "echo")],
        );

        let output = executor
            .execute(
                "echoer",
                &serde_json::json!({"args": ["hello"]}),
                &test_ctx(),
            )
            .expect("manifest-backed tool should execute successfully");
        assert_eq!(output.trim(), "hello");
    }

    #[test]
    fn unmatched_tool_call_falls_back_to_inner_executor() {
        let executor = ManifestToolExecutor::new(
            Box::new(StubToolExecutor {
                response: "from inner".to_string(),
            }),
            Box::new(PluginsAdapter::system()),
            vec![tool_manifest("echoer", "echo")],
        );

        let output = executor
            .execute("not_a_manifest_tool", &serde_json::json!({}), &test_ctx())
            .expect("fallback executor should handle unmatched names");
        assert_eq!(output, "from inner");
    }
}
