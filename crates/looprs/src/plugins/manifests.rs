use looprs_core::ports::{
    OrchestrationPluginPort, OrchestrationSupervisorPort, PluginAgentSelection,
    PluginExecutionMode, PluginHealthState, PluginKind, PluginSupervisorError,
    PluginSupervisorPort, PluginSupervisorStatus, RuntimeSupervisorPort, ToolSupervisorPort,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::api::ToolDefinition;
use crate::rules::{ExecutionBoundary, ExecutionRequest, RuleRegistry};

const DEFAULT_PLUGIN_TIMEOUT_MS: u64 = 5_000;

/// Wire protocol used to communicate with an executable plugin.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PluginProtocol {
    /// The process is supervised without request/response communication.
    #[default]
    None,
    /// Newline-delimited JSON messages over stdin and stdout.
    JsonLines,
}

/// Tool schema contributed by a tool plugin manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginToolManifest {
    /// Globally visible tool name.
    pub name: String,
    /// Description advertised to inference providers.
    pub description: String,
    /// JSON Schema accepted by the plugin tool.
    #[serde(default = "default_input_schema")]
    pub input_schema: Value,
}

fn default_input_schema() -> Value {
    serde_json::json!({"type": "object"})
}

fn default_plugin_timeout_ms() -> u64 {
    DEFAULT_PLUGIN_TIMEOUT_MS
}

/// Executable configuration for a managed plugin daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginEntry {
    /// Executable used to launch the plugin process. Arguments belong in `args`.
    pub command: String,
    /// Arguments passed directly to the executable without shell expansion.
    #[serde(default)]
    pub args: Vec<String>,
    /// Optional command used to verify daemon health after liveness succeeds.
    #[serde(default)]
    pub probe: Option<PluginProbe>,
    /// Optional stdin/stdout process protocol.
    #[serde(default)]
    pub protocol: PluginProtocol,
    /// Startup and request timeout in milliseconds.
    #[serde(default = "default_plugin_timeout_ms")]
    pub timeout_ms: u64,
}

/// Optional executable health check for a managed plugin daemon.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginProbe {
    /// Probe executable.
    pub command: String,
    /// Probe arguments passed without shell expansion.
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    /// Stable plugin identifier.
    pub name: String,
    /// Plugin category used for routing and policy decisions.
    pub kind: PluginKind,
    #[serde(default)]
    /// Optional description shown in diagnostics and listings.
    pub description: Option<String>,
    #[serde(default = "enabled_by_default")]
    /// Whether the plugin is active.
    pub enabled: bool,
    #[serde(default)]
    /// Whether missing/invalid plugin state should fail closed.
    pub required: bool,
    #[serde(default)]
    /// Execution mode (`one_shot` or `daemon`).
    pub mode: PluginExecutionMode,
    #[serde(default)]
    /// Runtime entrypoint details when the plugin is executable.
    pub entry: Option<PluginEntry>,
    #[serde(default)]
    /// Prompt triggers that activate this plugin.
    pub triggers: Vec<String>,
    #[serde(default)]
    /// Target agent name for orchestration plugins.
    pub route_to_agent: Option<String>,
    #[serde(default)]
    /// Tool schemas registered by a tool plugin.
    pub tools: Vec<PluginToolManifest>,
}

fn enabled_by_default() -> bool {
    true
}

#[derive(Debug, Clone, Default)]
pub struct PluginManifestRegistry {
    manifests: HashMap<(PluginKind, String), PluginManifest>,
}

impl PluginManifestRegistry {
    /// Create an empty manifest registry.
    pub fn new() -> Self {
        Self {
            manifests: HashMap::new(),
        }
    }

    /// Insert or replace one manifest keyed by `(kind, name)`.
    pub fn register(&mut self, manifest: PluginManifest) {
        self.manifests
            .insert((manifest.kind, manifest.name.clone()), manifest);
    }

    /// Fetch a manifest by kind and name.
    pub fn get(&self, kind: PluginKind, name: &str) -> Option<&PluginManifest> {
        self.manifests.get(&(kind, name.to_string()))
    }

    /// List manifests for a specific kind, sorted by name.
    pub fn list_by_kind(&self, kind: PluginKind) -> Vec<&PluginManifest> {
        let mut items: Vec<&PluginManifest> = self
            .manifests
            .iter()
            .filter_map(|((k, _), v)| (*k == kind).then_some(v))
            .collect();
        items.sort_by_key(|m| &m.name);
        items
    }

    /// Load all YAML manifests from one directory.
    pub fn load_from_directory(dir: &PathBuf) -> anyhow::Result<Self> {
        let mut registry = Self::new();
        if !dir.exists() {
            return Ok(registry);
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let ext = path.extension().and_then(|s| s.to_str());
            if ext != Some("yaml") && ext != Some("yml") {
                continue;
            }

            let manifest = Self::parse_manifest(&path).map_err(|error| {
                anyhow::anyhow!("failed to load plugin {}: {error}", path.display())
            })?;
            registry.register(manifest);
        }

        Ok(registry)
    }

    /// Merge user and repo manifest sources with repo precedence.
    pub fn load_dual_source(
        user_dir: Option<&PathBuf>,
        repo_dir: Option<&PathBuf>,
    ) -> anyhow::Result<Self> {
        let mut registry = Self::new();

        if let Some(user_path) = user_dir {
            let user = Self::load_from_directory(user_path)?;
            for manifest in user.manifests.values() {
                registry.register(manifest.clone());
            }
        }

        if let Some(repo_path) = repo_dir {
            let repo = Self::load_from_directory(repo_path)?;
            for manifest in repo.manifests.values() {
                registry.register(manifest.clone());
            }
        }

        Ok(registry)
    }

    fn parse_manifest(path: &Path) -> anyhow::Result<PluginManifest> {
        let content = fs::read_to_string(path)?;
        let manifest: PluginManifest = serde_yaml::from_str(&content)?;
        if manifest.name.trim().is_empty() {
            anyhow::bail!("Plugin name cannot be empty");
        }
        if manifest.enabled
            && manifest.mode == PluginExecutionMode::Daemon
            && manifest
                .entry
                .as_ref()
                .is_none_or(|entry| entry.command.trim().is_empty())
        {
            anyhow::bail!(
                "Daemon plugin '{}' must define a non-empty entry.command",
                manifest.name
            );
        }
        if manifest
            .entry
            .as_ref()
            .and_then(|entry| entry.probe.as_ref())
            .is_some_and(|probe| probe.command.trim().is_empty())
        {
            anyhow::bail!(
                "Plugin '{}' must define a non-empty entry.probe.command",
                manifest.name
            );
        }
        if manifest
            .entry
            .as_ref()
            .is_some_and(|entry| entry.timeout_ms == 0)
        {
            anyhow::bail!(
                "Plugin '{}' timeout_ms must be greater than zero",
                manifest.name
            );
        }
        if manifest.kind != PluginKind::Tool && !manifest.tools.is_empty() {
            anyhow::bail!("Only tool plugins may declare tools");
        }
        if !manifest.tools.is_empty()
            && manifest
                .entry
                .as_ref()
                .is_none_or(|entry| entry.protocol != PluginProtocol::JsonLines)
        {
            anyhow::bail!(
                "Tool plugin '{}' must use the json_lines protocol",
                manifest.name
            );
        }
        let mut tool_names = HashSet::new();
        for tool in &manifest.tools {
            if tool.name.trim().is_empty() || !tool_names.insert(tool.name.as_str()) {
                anyhow::bail!(
                    "Plugin '{}' contains an empty or duplicate tool name",
                    manifest.name
                );
            }
        }
        if manifest.kind == PluginKind::Orchestration
            && manifest.route_to_agent.is_some()
            && manifest.triggers.is_empty()
        {
            anyhow::bail!("Orchestration plugins that route to agents must define triggers");
        }
        Ok(manifest)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RegistryFingerprint {
    file_count: usize,
    latest_modified_nanos: u128,
}

/// Maximum explicit or configuration-driven restarts for one daemon instance.
pub const MAX_PLUGIN_RESTARTS: u32 = 3;

/// Cooperative cancellation handle for a plugin request.
#[derive(Debug, Clone, Default)]
pub struct PluginCancellation {
    cancelled: Arc<AtomicBool>,
}

impl PluginCancellation {
    /// Create a request cancellation handle.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark the associated request as cancelled.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// Per-request timeout and cancellation controls.
#[derive(Debug, Clone, Default)]
pub struct PluginCallOptions {
    timeout: Option<Duration>,
    cancellation: PluginCancellation,
}

impl PluginCallOptions {
    /// Override the timeout declared by the plugin manifest.
    pub fn with_timeout(timeout: Duration) -> Self {
        Self {
            timeout: Some(timeout),
            ..Self::default()
        }
    }

    /// Attach a cooperative cancellation handle.
    pub fn with_cancellation(mut self, cancellation: PluginCancellation) -> Self {
        self.cancellation = cancellation;
        self
    }

    fn timeout(&self, entry: &PluginEntry) -> Duration {
        self.timeout
            .unwrap_or_else(|| Duration::from_millis(entry.timeout_ms))
    }
}

#[derive(Debug)]
struct ManagedPlugin {
    status: PluginSupervisorStatus,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    responses: Option<Mutex<Receiver<Result<String, String>>>>,
    stderr: Arc<Mutex<String>>,
}

impl ManagedPlugin {
    fn disabled(kind: PluginKind, plugin_name: &str) -> Self {
        Self {
            status: PluginSupervisorStatus {
                plugin_name: plugin_name.to_string(),
                kind,
                state: PluginHealthState::Disabled,
                restart_count: 0,
                pid: None,
                last_error: None,
                last_restart_reason: None,
            },
            child: None,
            stdin: None,
            responses: None,
            stderr: Arc::new(Mutex::new(String::new())),
        }
    }

    fn unhealthy(kind: PluginKind, plugin_name: &str, message: String) -> Self {
        Self {
            status: PluginSupervisorStatus {
                plugin_name: plugin_name.to_string(),
                kind,
                state: PluginHealthState::Unhealthy,
                restart_count: 0,
                pid: None,
                last_error: Some(message),
                last_restart_reason: None,
            },
            child: None,
            stdin: None,
            responses: None,
            stderr: Arc::new(Mutex::new(String::new())),
        }
    }

    fn refresh_liveness(&mut self) -> Result<(), String> {
        let Some(child) = self.child.as_mut() else {
            return Ok(());
        };
        match child.try_wait() {
            Ok(Some(exit)) => {
                self.status.state = PluginHealthState::Unhealthy;
                self.status.pid = None;
                self.status.last_error = Some(format!("process exited with status {exit}"));
                self.child = None;
                self.stdin = None;
                self.responses = None;
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(error) => {
                self.status.state = PluginHealthState::Unhealthy;
                self.status.last_error = Some(error.to_string());
                Err(error.to_string())
            }
        }
    }

    fn shutdown(&mut self) -> Result<(), String> {
        if let Some(mut child) = self.child.take()
            && child
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_none()
        {
            child.kill().map_err(|error| error.to_string())?;
            child.wait().map_err(|error| error.to_string())?;
        }
        self.status.state = PluginHealthState::Stopped;
        self.status.pid = None;
        self.status.last_error = None;
        self.stdin = None;
        self.responses = None;
        Ok(())
    }

    fn request(
        &mut self,
        request: &Value,
        timeout: Duration,
        cancellation: &PluginCancellation,
    ) -> anyhow::Result<Value> {
        self.refresh_liveness().map_err(anyhow::Error::msg)?;
        if self.status.state != PluginHealthState::Healthy {
            anyhow::bail!(
                "plugin '{}' is not healthy: {}",
                self.status.plugin_name,
                self.status
                    .last_error
                    .as_deref()
                    .unwrap_or("process is not running")
            );
        }
        if cancellation.is_cancelled() {
            anyhow::bail!("plugin request cancelled before execution");
        }
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("plugin protocol stdin is unavailable"))?;
        serde_json::to_writer(&mut *stdin, request)?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;

        let responses = self
            .responses
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("plugin protocol stdout is unavailable"))?;
        let responses = responses
            .lock()
            .map_err(|_| anyhow::anyhow!("plugin response channel lock poisoned"))?;
        let started = Instant::now();
        loop {
            if cancellation.is_cancelled() {
                anyhow::bail!("plugin request cancelled");
            }
            let remaining = timeout.saturating_sub(started.elapsed());
            if remaining.is_zero() {
                let stderr = self.stderr.lock().map(|s| s.clone()).unwrap_or_default();
                self.status.state = PluginHealthState::Unhealthy;
                self.status.last_error = Some("protocol response timed out".to_string());
                anyhow::bail!("plugin request timed out; stderr: {stderr}");
            }
            match responses.recv_timeout(remaining.min(Duration::from_millis(10))) {
                Ok(Ok(line)) => return parse_protocol_response(&line),
                Ok(Err(error)) => anyhow::bail!("plugin output failed: {error}"),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    drop(responses);
                    self.refresh_liveness().map_err(anyhow::Error::msg)?;
                    let stderr = self.stderr.lock().map(|s| s.clone()).unwrap_or_default();
                    anyhow::bail!("plugin process closed stdout; stderr: {stderr}");
                }
            }
        }
    }
}

impl Drop for ManagedPlugin {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

#[derive(Debug, Default)]
struct KindSupervisor {
    managed: HashMap<String, ManagedPlugin>,
}

#[derive(Debug, Default)]
struct ReconcilePlan {
    replacements: Vec<(String, ManagedPlugin)>,
    removals: Vec<String>,
}

fn spawn_line_reader<R>(reader: R) -> Receiver<Result<String, String>>
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            let item = line.map_err(|error| error.to_string());
            if sender.send(item).is_err() {
                break;
            }
        }
    });
    receiver
}

fn spawn_stderr_reader<R>(reader: R, output: Arc<Mutex<String>>)
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        for line in BufReader::new(reader).lines() {
            let Ok(line) = line else {
                break;
            };
            if let Ok(mut captured) = output.lock() {
                if !captured.is_empty() {
                    captured.push('\n');
                }
                captured.push_str(&line);
            }
        }
    });
}

fn parse_ready_message(line: &str) -> Result<(), String> {
    let value: Value = serde_json::from_str(line)
        .map_err(|error| format!("invalid readiness message: {error}"))?;
    if value.get("type").and_then(Value::as_str) == Some("ready") {
        Ok(())
    } else {
        Err("readiness message must have type 'ready'".to_string())
    }
}

fn parse_protocol_response(line: &str) -> anyhow::Result<Value> {
    let value: Value = serde_json::from_str(line)
        .map_err(|error| anyhow::anyhow!("invalid plugin response: {error}"))?;
    if value.get("type").and_then(Value::as_str) != Some("response") {
        anyhow::bail!("plugin response must have type 'response'");
    }
    if let Some(error) = value.get("error") {
        anyhow::bail!("plugin returned error: {error}");
    }
    value
        .get("result")
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("plugin response is missing result"))
}

impl KindSupervisor {
    fn resolve_command(command: &str) -> Option<PathBuf> {
        let path = Path::new(command);
        if path.components().count() > 1 {
            path.is_file().then(|| path.to_path_buf())
        } else {
            crate::plugins::resolve::find_in_path(command)
        }
    }

    fn launch(
        kind: PluginKind,
        manifest: &PluginManifest,
        restart_count: u32,
        restart_reason: Option<String>,
    ) -> Result<ManagedPlugin, PluginSupervisorError> {
        if !manifest.enabled {
            return Ok(ManagedPlugin::disabled(kind, &manifest.name));
        }
        let entry = manifest
            .entry
            .as_ref()
            .ok_or_else(|| PluginSupervisorError::LaunchFailed {
                kind,
                plugin_name: manifest.name.clone(),
                message: "missing entry.command".to_string(),
            })?;
        let program = Self::resolve_command(entry.command.trim()).ok_or_else(|| {
            PluginSupervisorError::LaunchFailed {
                kind,
                plugin_name: manifest.name.clone(),
                message: format!("executable '{}' was not found", entry.command),
            }
        })?;
        let mut command = Command::new(program);
        command.args(&entry.args);
        if entry.protocol == PluginProtocol::JsonLines {
            command
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
        } else {
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
        }
        let mut child = command
            .spawn()
            .map_err(|error| PluginSupervisorError::LaunchFailed {
                kind,
                plugin_name: manifest.name.clone(),
                message: error.to_string(),
            })?;
        let pid = child.id();
        let stderr = Arc::new(Mutex::new(String::new()));
        let (stdin, responses) = if entry.protocol == PluginProtocol::JsonLines {
            let stdin = child
                .stdin
                .take()
                .ok_or_else(|| PluginSupervisorError::LaunchFailed {
                    kind,
                    plugin_name: manifest.name.clone(),
                    message: "protocol stdin was not piped".to_string(),
                })?;
            let stdout =
                child
                    .stdout
                    .take()
                    .ok_or_else(|| PluginSupervisorError::LaunchFailed {
                        kind,
                        plugin_name: manifest.name.clone(),
                        message: "protocol stdout was not piped".to_string(),
                    })?;
            let child_stderr =
                child
                    .stderr
                    .take()
                    .ok_or_else(|| PluginSupervisorError::LaunchFailed {
                        kind,
                        plugin_name: manifest.name.clone(),
                        message: "protocol stderr was not piped".to_string(),
                    })?;
            spawn_stderr_reader(child_stderr, Arc::clone(&stderr));
            let responses = spawn_line_reader(stdout);
            let timeout = Duration::from_millis(entry.timeout_ms);
            let readiness = responses
                .recv_timeout(timeout)
                .map_err(|error| format!("readiness timed out or disconnected: {error}"))
                .and_then(|line| line.and_then(|line| parse_ready_message(&line)));
            if let Err(message) = readiness {
                let _ = child.kill();
                let _ = child.wait();
                return Err(PluginSupervisorError::LaunchFailed {
                    kind,
                    plugin_name: manifest.name.clone(),
                    message,
                });
            }
            (Some(stdin), Some(Mutex::new(responses)))
        } else {
            (None, None)
        };
        Ok(ManagedPlugin {
            status: PluginSupervisorStatus {
                plugin_name: manifest.name.clone(),
                kind,
                state: PluginHealthState::Healthy,
                restart_count,
                pid: Some(pid),
                last_error: None,
                last_restart_reason: restart_reason,
            },
            child: Some(child),
            stdin,
            responses,
            stderr,
        })
    }

    fn plan_reconcile(
        &self,
        kind: PluginKind,
        old: &PluginManifestRegistry,
        new: &PluginManifestRegistry,
    ) -> Result<ReconcilePlan, PluginSupervisorError> {
        let old_map: HashMap<&str, &PluginManifest> = old
            .list_by_kind(kind)
            .into_iter()
            .map(|m| (m.name.as_str(), m))
            .collect();
        let new_map: HashMap<&str, &PluginManifest> = new
            .list_by_kind(kind)
            .into_iter()
            .map(|m| (m.name.as_str(), m))
            .collect();

        let mut replacements = Vec::new();
        for (name, manifest) in &new_map {
            if manifest.mode != PluginExecutionMode::Daemon {
                continue;
            }
            let previous = old_map.get(name);
            if previous.is_some_and(|old_manifest| *old_manifest == *manifest)
                && self.managed.contains_key(*name)
            {
                continue;
            }
            let restart_count = match self.managed.get(*name) {
                Some(process) => process.status.restart_count.checked_add(1).ok_or_else(|| {
                    PluginSupervisorError::RestartLimitReached {
                        kind,
                        plugin_name: (*name).to_string(),
                        limit: MAX_PLUGIN_RESTARTS,
                    }
                })?,
                None => 0,
            };
            if restart_count > MAX_PLUGIN_RESTARTS {
                return Err(PluginSupervisorError::RestartLimitReached {
                    kind,
                    plugin_name: (*name).to_string(),
                    limit: MAX_PLUGIN_RESTARTS,
                });
            }
            let launched = Self::launch(
                kind,
                manifest,
                restart_count,
                (restart_count > 0).then(|| "manifest changed".to_string()),
            );
            let process = match launched {
                Ok(process) => process,
                Err(error) if !manifest.required => {
                    ManagedPlugin::unhealthy(kind, name, error.to_string())
                }
                Err(error) => return Err(error),
            };
            replacements.push(((*name).to_string(), process));
        }

        let removals = self
            .managed
            .keys()
            .filter(|name| {
                new_map.get(name.as_str()).is_none_or(|manifest| {
                    manifest.mode != PluginExecutionMode::Daemon
                        || old_map
                            .get(name.as_str())
                            .is_none_or(|old_manifest| *old_manifest != *manifest)
                })
            })
            .cloned()
            .collect::<Vec<_>>();
        Ok(ReconcilePlan {
            replacements,
            removals,
        })
    }

    fn apply_reconcile(&mut self, plan: ReconcilePlan) {
        for name in plan.removals {
            self.managed.remove(&name);
        }
        for (name, process) in plan.replacements {
            self.managed.insert(name, process);
        }
    }
}

#[derive(Debug, Default)]
pub struct PluginRuntimeRegistry {
    user_dir: Option<PathBuf>,
    repo_dir: Option<PathBuf>,
    fingerprint: Option<RegistryFingerprint>,
    registry: PluginManifestRegistry,
    tool_supervisor: KindSupervisor,
    runtime_supervisor: KindSupervisor,
    orchestration_supervisor: KindSupervisor,
    diagnostics: Vec<String>,
}

impl PluginRuntimeRegistry {
    /// Build runtime registry from optional user and repository manifest directories.
    pub fn load_dual_source(
        user_dir: Option<PathBuf>,
        repo_dir: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
        let mut runtime = Self {
            user_dir,
            repo_dir,
            ..Self::default()
        };
        runtime.reload_now()?;
        Ok(runtime)
    }

    /// List orchestration manifests visible to the runtime.
    pub fn list_orchestration_plugins(&self) -> Vec<&PluginManifest> {
        self.registry.list_by_kind(PluginKind::Orchestration)
    }

    /// Fetch a single orchestration plugin by name.
    pub fn orchestration_plugin(&self, name: &str) -> Option<&PluginManifest> {
        self.registry.get(PluginKind::Orchestration, name)
    }

    /// Return diagnostics collected while optional plugins degraded.
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    /// Return tool schemas contributed by healthy executable manifests.
    pub fn tool_definitions(&mut self) -> anyhow::Result<Vec<ToolDefinition>> {
        let _ = self.refresh_if_configured()?;
        let manifests = self
            .registry
            .list_by_kind(PluginKind::Tool)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        let mut definitions = Vec::new();
        let mut names = HashSet::new();
        for manifest in manifests {
            if !manifest.enabled || manifest.tools.is_empty() {
                continue;
            }
            let available = self.plugin_is_available(&manifest);
            if let Err(error) = available {
                if manifest.required {
                    return Err(error);
                }
                self.diagnostics.push(format!(
                    "optional plugin '{}' unavailable: {error}",
                    manifest.name
                ));
                continue;
            }
            for tool in manifest.tools {
                if !names.insert(tool.name.clone()) {
                    anyhow::bail!("duplicate plugin tool name '{}'", tool.name);
                }
                definitions.push(ToolDefinition {
                    name: tool.name,
                    description: tool.description,
                    input_schema: tool.input_schema,
                });
            }
        }
        Ok(definitions)
    }

    /// Execute a registered manifest tool through the runtime policy boundary.
    pub fn execute_tool(
        &mut self,
        tool_name: &str,
        input: &Value,
        rules: &RuleRegistry,
        options: &PluginCallOptions,
    ) -> anyhow::Result<Value> {
        let _ = self.refresh_if_configured()?;
        let manifest = self
            .registry
            .list_by_kind(PluginKind::Tool)
            .into_iter()
            .find(|manifest| manifest.tools.iter().any(|tool| tool.name == tool_name))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unknown manifest plugin tool '{tool_name}'"))?;
        let request = ExecutionRequest::new(
            ExecutionBoundary::Plugin,
            tool_name,
            serde_json::to_string(input)?,
        );
        rules.authorize(&request, false)?;
        self.execute_manifest(&manifest, "tool/call", tool_name, input, options)
    }

    /// Execute a named runtime plugin in one-shot or daemon mode.
    pub fn execute_runtime(
        &mut self,
        plugin_name: &str,
        input: &Value,
        rules: &RuleRegistry,
        options: &PluginCallOptions,
    ) -> anyhow::Result<Value> {
        let _ = self.refresh_if_configured()?;
        let manifest = self
            .registry
            .get(PluginKind::Runtime, plugin_name)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("unknown runtime plugin '{plugin_name}'"))?;
        let request = ExecutionRequest::new(
            ExecutionBoundary::Plugin,
            plugin_name,
            serde_json::to_string(input)?,
        );
        rules.authorize(&request, false)?;
        self.execute_manifest(&manifest, "runtime/run", plugin_name, input, options)
    }

    fn plugin_is_available(&mut self, manifest: &PluginManifest) -> anyhow::Result<()> {
        let entry = manifest
            .entry
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing entry.command"))?;
        if entry.protocol != PluginProtocol::JsonLines {
            anyhow::bail!("json_lines protocol is required for execution");
        }
        if manifest.mode == PluginExecutionMode::Daemon {
            let status = self.status(manifest.kind, &manifest.name)?;
            if status.state != PluginHealthState::Healthy {
                anyhow::bail!(
                    "plugin process is {:?}: {}",
                    status.state,
                    status.last_error.as_deref().unwrap_or("unknown error")
                );
            }
        } else if KindSupervisor::resolve_command(entry.command.trim()).is_none() {
            anyhow::bail!("executable '{}' was not found", entry.command);
        }
        Ok(())
    }

    fn refresh_if_configured(&mut self) -> anyhow::Result<bool> {
        if self.user_dir.is_none() && self.repo_dir.is_none() {
            return Ok(false);
        }
        self.refresh_if_changed()
    }

    fn execute_manifest(
        &mut self,
        manifest: &PluginManifest,
        method: &str,
        request_name: &str,
        input: &Value,
        options: &PluginCallOptions,
    ) -> anyhow::Result<Value> {
        if !manifest.enabled {
            anyhow::bail!("plugin '{}' is disabled", manifest.name);
        }
        let entry = manifest
            .entry
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("plugin '{}' has no entry", manifest.name))?;
        if entry.protocol != PluginProtocol::JsonLines {
            anyhow::bail!("plugin '{}' does not use json_lines", manifest.name);
        }
        let request = serde_json::json!({
            "type": "request",
            "id": "1",
            "method": method,
            "name": request_name,
            "input": input,
        });
        let timeout = options.timeout(entry);
        let result = match manifest.mode {
            PluginExecutionMode::OneShot => {
                Self::execute_oneshot(manifest, entry, &request, timeout, &options.cancellation)
            }
            PluginExecutionMode::Daemon => {
                let process = self
                    .supervisor_mut(manifest.kind)
                    .managed
                    .get_mut(&manifest.name)
                    .ok_or_else(|| anyhow::anyhow!("plugin daemon is not managed"))?;
                process.request(&request, timeout, &options.cancellation)
            }
        };
        if let Err(error) = &result
            && !manifest.required
        {
            self.diagnostics.push(format!(
                "optional plugin '{}' request failed: {error}",
                manifest.name
            ));
        }
        result
    }

    fn execute_oneshot(
        manifest: &PluginManifest,
        entry: &PluginEntry,
        request: &Value,
        timeout: Duration,
        cancellation: &PluginCancellation,
    ) -> anyhow::Result<Value> {
        if cancellation.is_cancelled() {
            anyhow::bail!("plugin request cancelled before launch");
        }
        let program = KindSupervisor::resolve_command(entry.command.trim())
            .ok_or_else(|| anyhow::anyhow!("executable '{}' was not found", entry.command))?;
        let mut child = Command::new(program)
            .args(&entry.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                anyhow::anyhow!("failed to launch plugin '{}': {error}", manifest.name)
            })?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow::anyhow!("plugin stdin was not piped"))?;
        serde_json::to_writer(&mut stdin, request)?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        drop(stdin);

        let started = Instant::now();
        loop {
            if cancellation.is_cancelled() {
                let _ = child.kill();
                let output = child.wait_with_output()?;
                anyhow::bail!(
                    "plugin request cancelled; stderr: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
            if started.elapsed() >= timeout {
                let _ = child.kill();
                let output = child.wait_with_output()?;
                anyhow::bail!(
                    "plugin request timed out; stderr: {}",
                    String::from_utf8_lossy(&output.stderr).trim()
                );
            }
            if child.try_wait()?.is_some() {
                let output = child.wait_with_output()?;
                if !output.status.success() {
                    anyhow::bail!(
                        "plugin exited with status {}; stderr: {}",
                        output.status,
                        String::from_utf8_lossy(&output.stderr).trim()
                    );
                }
                let stdout = String::from_utf8(output.stdout)?;
                let line = stdout
                    .lines()
                    .find(|line| !line.trim().is_empty())
                    .ok_or_else(|| anyhow::anyhow!("plugin returned no protocol response"))?;
                return parse_protocol_response(line);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    fn supervisor_mut(&mut self, kind: PluginKind) -> &mut KindSupervisor {
        match kind {
            PluginKind::Tool => &mut self.tool_supervisor,
            PluginKind::Runtime => &mut self.runtime_supervisor,
            PluginKind::Orchestration => &mut self.orchestration_supervisor,
        }
    }

    fn supervisor(&self, kind: PluginKind) -> &KindSupervisor {
        match kind {
            PluginKind::Tool => &self.tool_supervisor,
            PluginKind::Runtime => &self.runtime_supervisor,
            PluginKind::Orchestration => &self.orchestration_supervisor,
        }
    }

    fn supervision_manifest(
        &self,
        kind: PluginKind,
        plugin_name: &str,
    ) -> Result<&PluginManifest, PluginSupervisorError> {
        let manifest = self.registry.get(kind, plugin_name).ok_or_else(|| {
            PluginSupervisorError::UnknownPlugin {
                kind,
                plugin_name: plugin_name.to_string(),
            }
        })?;
        if manifest.mode != PluginExecutionMode::Daemon {
            return Err(PluginSupervisorError::NotDaemon {
                kind,
                plugin_name: plugin_name.to_string(),
            });
        }
        Ok(manifest)
    }

    /// Reload manifests when file fingerprints changed.
    pub fn refresh_if_changed(&mut self) -> anyhow::Result<bool> {
        let fingerprint = self.compute_fingerprint()?;
        if self.fingerprint.as_ref() == Some(&fingerprint) {
            return Ok(false);
        }
        self.reload_now()?;
        Ok(true)
    }

    fn reload_now(&mut self) -> anyhow::Result<()> {
        let fingerprint = self.compute_fingerprint()?;
        let old = self.registry.clone();
        let next = PluginManifestRegistry::load_dual_source(
            self.user_dir.as_ref(),
            self.repo_dir.as_ref(),
        )?;

        let tool_plan = self
            .tool_supervisor
            .plan_reconcile(PluginKind::Tool, &old, &next)?;
        let runtime_plan =
            self.runtime_supervisor
                .plan_reconcile(PluginKind::Runtime, &old, &next)?;
        let orchestration_plan =
            self.orchestration_supervisor
                .plan_reconcile(PluginKind::Orchestration, &old, &next)?;

        self.tool_supervisor.apply_reconcile(tool_plan);
        self.runtime_supervisor.apply_reconcile(runtime_plan);
        self.orchestration_supervisor
            .apply_reconcile(orchestration_plan);

        self.registry = next;
        let mut diagnostics = Vec::new();
        for kind in [
            PluginKind::Tool,
            PluginKind::Runtime,
            PluginKind::Orchestration,
        ] {
            for manifest in self.registry.list_by_kind(kind) {
                if manifest.required {
                    continue;
                }
                if let Some(process) = self.supervisor(kind).managed.get(&manifest.name)
                    && process.status.state == PluginHealthState::Unhealthy
                {
                    diagnostics.push(format!(
                        "optional plugin '{}' readiness failed: {}",
                        manifest.name,
                        process
                            .status
                            .last_error
                            .as_deref()
                            .unwrap_or("unknown error")
                    ));
                }
            }
        }
        self.diagnostics = diagnostics;
        self.fingerprint = Some(fingerprint);
        Ok(())
    }

    fn compute_fingerprint(&self) -> anyhow::Result<RegistryFingerprint> {
        let mut file_count = 0usize;
        let mut latest_modified_nanos = 0u128;

        for dir in [&self.user_dir, &self.repo_dir].into_iter().flatten() {
            if !dir.exists() {
                continue;
            }

            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                let ext = path.extension().and_then(|s| s.to_str());
                if ext != Some("yaml") && ext != Some("yml") {
                    continue;
                }
                file_count = file_count.saturating_add(1);
                let modified = entry
                    .metadata()?
                    .modified()
                    .unwrap_or(SystemTime::UNIX_EPOCH);
                let nanos = modified
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                latest_modified_nanos = latest_modified_nanos.max(nanos);
            }
        }

        Ok(RegistryFingerprint {
            file_count,
            latest_modified_nanos,
        })
    }

    fn trigger_matches_prompt(prompt_lower: &str, trigger: &str) -> bool {
        let trigger_lower = trigger.to_lowercase();
        if trigger_lower.trim().is_empty() {
            return false;
        }

        if trigger_lower.chars().any(char::is_whitespace) {
            return prompt_lower.contains(&trigger_lower);
        }

        let mut start = 0usize;
        while let Some(found) = prompt_lower[start..].find(&trigger_lower) {
            let abs = start + found;
            let end = abs + trigger_lower.len();

            let before_ok = abs == 0
                || !prompt_lower[..abs]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
            let after_ok = end == prompt_lower.len()
                || !prompt_lower[end..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');

            if before_ok && after_ok {
                return true;
            }

            start = abs + 1;
        }

        false
    }
}

/// Agent tool-port adapter backed by executable plugin manifests.
#[derive(Clone)]
pub struct ManifestToolRuntime {
    runtime: Arc<Mutex<PluginRuntimeRegistry>>,
    rules: Arc<RuleRegistry>,
    fallback_catalog: Arc<dyn crate::tools::ToolCatalog>,
    fallback_dispatcher: Arc<dyn crate::tools::ToolDispatcher>,
}

impl std::fmt::Debug for ManifestToolRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ManifestToolRuntime")
            .finish_non_exhaustive()
    }
}

impl ManifestToolRuntime {
    /// Compose manifest-backed tools with existing catalog and dispatcher fallbacks.
    pub fn new(
        runtime: Arc<Mutex<PluginRuntimeRegistry>>,
        rules: Arc<RuleRegistry>,
        fallback_catalog: Arc<dyn crate::tools::ToolCatalog>,
        fallback_dispatcher: Arc<dyn crate::tools::ToolDispatcher>,
    ) -> Self {
        Self {
            runtime,
            rules,
            fallback_catalog,
            fallback_dispatcher,
        }
    }

    /// Convert this adapter into injectable agent tool ports.
    pub fn into_tool_ports(self) -> crate::tools::ToolPorts {
        let shared = Arc::new(self);
        crate::tools::ToolPorts::new(shared.clone(), shared)
    }
}

#[async_trait::async_trait]
impl crate::tools::ToolCatalog for ManifestToolRuntime {
    async fn definitions(&self) -> anyhow::Result<Vec<ToolDefinition>> {
        let mut definitions = self.fallback_catalog.definitions().await?;
        let mut known = definitions
            .iter()
            .map(|definition| definition.name.clone())
            .collect::<HashSet<_>>();
        let plugin_definitions = self
            .runtime
            .lock()
            .map_err(|_| anyhow::anyhow!("plugin runtime lock poisoned"))?
            .tool_definitions()?;
        definitions.extend(
            plugin_definitions
                .into_iter()
                .filter(|definition| known.insert(definition.name.clone())),
        );
        Ok(definitions)
    }
}

#[async_trait::async_trait]
impl crate::tools::ToolDispatcher for ManifestToolRuntime {
    async fn execute(
        &self,
        name: &str,
        args: &Value,
        ctx: &crate::tools::ToolContext,
    ) -> Result<String, crate::tools::ToolError> {
        let is_plugin_tool = {
            let runtime = self.runtime.lock().map_err(|_| {
                crate::tools::ToolError::CommandFailed("plugin runtime lock poisoned".to_string())
            })?;
            runtime
                .registry
                .list_by_kind(PluginKind::Tool)
                .iter()
                .any(|manifest| manifest.tools.iter().any(|tool| tool.name == name))
        };
        if !is_plugin_tool {
            return self.fallback_dispatcher.execute(name, args, ctx).await;
        }
        let output = self
            .runtime
            .lock()
            .map_err(|_| {
                crate::tools::ToolError::CommandFailed("plugin runtime lock poisoned".to_string())
            })?
            .execute_tool(name, args, &self.rules, &PluginCallOptions::default())
            .map_err(|error| crate::tools::ToolError::CommandFailed(error.to_string()))?;
        serde_json::to_string(&output)
            .map_err(|error| crate::tools::ToolError::CommandFailed(error.to_string()))
    }
}

impl OrchestrationPluginPort for PluginRuntimeRegistry {
    fn select_agent_for_prompt(
        &mut self,
        prompt: &str,
    ) -> anyhow::Result<Option<PluginAgentSelection>> {
        let _ = self.refresh_if_changed()?;

        let lower = prompt.to_lowercase();
        let manifests = self
            .registry
            .list_by_kind(PluginKind::Orchestration)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();
        for manifest in manifests {
            if !manifest.enabled || manifest.triggers.is_empty() {
                continue;
            }
            let matched = manifest
                .triggers
                .iter()
                .any(|trigger| Self::trigger_matches_prompt(&lower, trigger));
            if !matched {
                continue;
            }

            if manifest.mode == PluginExecutionMode::Daemon {
                match self.probe(PluginKind::Orchestration, &manifest.name) {
                    Ok(status) if status.state == PluginHealthState::Healthy => {}
                    Ok(status) if manifest.required => {
                        anyhow::bail!(
                            "Required orchestration plugin '{}' is {:?}",
                            manifest.name,
                            status.state
                        );
                    }
                    Ok(_) => continue,
                    Err(error) if manifest.required => return Err(error.into()),
                    Err(_) => continue,
                }
            }

            if let Some(agent_name) = &manifest.route_to_agent {
                return Ok(Some(PluginAgentSelection {
                    plugin_name: manifest.name.clone(),
                    agent_name: agent_name.clone(),
                }));
            }

            if manifest.required {
                anyhow::bail!(
                    "Required orchestration plugin '{}' matched prompt but has no route_to_agent",
                    manifest.name
                );
            }
        }

        Ok(None)
    }
}

impl PluginSupervisorPort for PluginRuntimeRegistry {
    fn status(
        &mut self,
        kind: PluginKind,
        plugin_name: &str,
    ) -> Result<PluginSupervisorStatus, PluginSupervisorError> {
        self.supervision_manifest(kind, plugin_name)?;
        let process = self
            .supervisor_mut(kind)
            .managed
            .get_mut(plugin_name)
            .ok_or_else(|| PluginSupervisorError::LaunchFailed {
                kind,
                plugin_name: plugin_name.to_string(),
                message: "daemon has no managed process state".to_string(),
            })?;
        process
            .refresh_liveness()
            .map_err(|message| PluginSupervisorError::ProbeFailed {
                kind,
                plugin_name: plugin_name.to_string(),
                message,
            })?;
        Ok(process.status.clone())
    }

    fn probe(
        &mut self,
        kind: PluginKind,
        plugin_name: &str,
    ) -> Result<PluginSupervisorStatus, PluginSupervisorError> {
        let manifest = self.supervision_manifest(kind, plugin_name)?.clone();
        if !manifest.enabled {
            return Err(PluginSupervisorError::Disabled {
                kind,
                plugin_name: plugin_name.to_string(),
            });
        }
        let status = self.status(kind, plugin_name)?;
        if status.state != PluginHealthState::Healthy {
            return Ok(status);
        }
        let Some(probe) = manifest.entry.and_then(|entry| entry.probe) else {
            return Ok(status);
        };
        let output = (|| {
            let program = KindSupervisor::resolve_command(probe.command.trim())
                .ok_or_else(|| format!("executable '{}' was not found", probe.command))?;
            Command::new(program)
                .args(&probe.args)
                .stdin(Stdio::null())
                .output()
                .map_err(|error| error.to_string())
        })();
        let output = match output {
            Ok(output) => output,
            Err(message) => {
                let process = self
                    .supervisor_mut(kind)
                    .managed
                    .get_mut(plugin_name)
                    .ok_or_else(|| PluginSupervisorError::ProbeFailed {
                        kind,
                        plugin_name: plugin_name.to_string(),
                        message: "daemon has no managed process state".to_string(),
                    })?;
                process.status.state = PluginHealthState::Unhealthy;
                process.status.last_error = Some(message.clone());
                return Err(PluginSupervisorError::ProbeFailed {
                    kind,
                    plugin_name: plugin_name.to_string(),
                    message,
                });
            }
        };
        if output.status.success() {
            return Ok(status);
        }
        let message = format!("probe exited with status {}", output.status);
        let process = self
            .supervisor_mut(kind)
            .managed
            .get_mut(plugin_name)
            .ok_or_else(|| PluginSupervisorError::ProbeFailed {
                kind,
                plugin_name: plugin_name.to_string(),
                message: "daemon has no managed process state".to_string(),
            })?;
        process.status.state = PluginHealthState::Unhealthy;
        process.status.last_error = Some(message);
        Ok(process.status.clone())
    }

    fn restart(
        &mut self,
        kind: PluginKind,
        plugin_name: &str,
        reason: &str,
    ) -> Result<(), PluginSupervisorError> {
        self.refresh_if_changed()
            .map_err(|error| PluginSupervisorError::RefreshFailed {
                message: error.to_string(),
            })?;
        let manifest = self.supervision_manifest(kind, plugin_name)?.clone();
        if !manifest.enabled {
            return Err(PluginSupervisorError::Disabled {
                kind,
                plugin_name: plugin_name.to_string(),
            });
        }
        let restart_count = self
            .supervisor_mut(kind)
            .managed
            .get(plugin_name)
            .map(|process| process.status.restart_count)
            .unwrap_or_default();
        if restart_count >= MAX_PLUGIN_RESTARTS {
            return Err(PluginSupervisorError::RestartLimitReached {
                kind,
                plugin_name: plugin_name.to_string(),
                limit: MAX_PLUGIN_RESTARTS,
            });
        }
        let replacement =
            KindSupervisor::launch(kind, &manifest, restart_count + 1, Some(reason.to_string()))?;
        self.supervisor_mut(kind)
            .managed
            .insert(plugin_name.to_string(), replacement);
        Ok(())
    }

    fn shutdown(
        &mut self,
        kind: PluginKind,
        plugin_name: &str,
    ) -> Result<(), PluginSupervisorError> {
        let manifest = self.supervision_manifest(kind, plugin_name)?;
        if !manifest.enabled {
            return Err(PluginSupervisorError::Disabled {
                kind,
                plugin_name: plugin_name.to_string(),
            });
        }
        self.supervisor_mut(kind)
            .managed
            .get_mut(plugin_name)
            .ok_or_else(|| PluginSupervisorError::LaunchFailed {
                kind,
                plugin_name: plugin_name.to_string(),
                message: "daemon has no managed process state".to_string(),
            })?
            .shutdown()
            .map_err(|message| PluginSupervisorError::ShutdownFailed {
                kind,
                plugin_name: plugin_name.to_string(),
                message,
            })
    }
}

impl ToolSupervisorPort for PluginRuntimeRegistry {}

impl RuntimeSupervisorPort for PluginRuntimeRegistry {}

impl OrchestrationSupervisorPort for PluginRuntimeRegistry {}

#[cfg(test)]
mod tests {
    use super::*;
    use looprs_core::ports::PluginSupervisorPort;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_plugin(dir: &Path, filename: &str, body: &str) {
        let path = dir.join(filename);
        let mut file = std::fs::File::create(path).unwrap();
        file.write_all(body.as_bytes()).unwrap();
    }

    #[test]
    fn repo_overrides_user_plugin_by_kind_and_name() {
        let user_dir = TempDir::new().unwrap();
        let repo_dir = TempDir::new().unwrap();

        write_plugin(
            user_dir.path(),
            "route.yaml",
            r#"name: route
kind: orchestration
triggers: ["health"]
route_to_agent: planner"#,
        );
        write_plugin(
            repo_dir.path(),
            "route.yaml",
            r#"name: route
kind: orchestration
triggers: ["health"]
route_to_agent: taskit"#,
        );

        let registry = PluginManifestRegistry::load_dual_source(
            Some(&user_dir.path().to_path_buf()),
            Some(&repo_dir.path().to_path_buf()),
        )
        .unwrap();

        let plugin = registry.get(PluginKind::Orchestration, "route").unwrap();
        assert_eq!(plugin.route_to_agent.as_deref(), Some("taskit"));
    }

    #[test]
    fn orchestration_plugin_selects_agent() {
        let repo_dir = TempDir::new().unwrap();
        write_plugin(
            repo_dir.path(),
            "route.yaml",
            r#"name: route
kind: orchestration
triggers: ["regression detected"]
route_to_agent: taskit"#,
        );

        let mut runtime =
            PluginRuntimeRegistry::load_dual_source(None, Some(repo_dir.path().to_path_buf()))
                .unwrap();
        let selected = runtime
            .select_agent_for_prompt("we have REGRESSION detected in CI")
            .unwrap()
            .unwrap();

        assert_eq!(selected.plugin_name, "route");
        assert_eq!(selected.agent_name, "taskit");
    }

    #[test]
    fn required_orchestration_plugin_without_route_fails() {
        let repo_dir = TempDir::new().unwrap();
        write_plugin(
            repo_dir.path(),
            "required.yaml",
            r#"name: required-route
kind: orchestration
required: true
triggers: ["route me"]"#,
        );

        let mut runtime =
            PluginRuntimeRegistry::load_dual_source(None, Some(repo_dir.path().to_path_buf()))
                .unwrap();
        let err = runtime
            .select_agent_for_prompt("please route me now")
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("Required orchestration plugin 'required-route'"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn daemon_supervision_conformance_for_all_plugin_kinds() {
        let repo_dir = TempDir::new().unwrap();
        for (filename, name, kind) in [
            ("tool.yaml", "daemon-tool", "tool"),
            ("runtime.yaml", "daemon-runtime", "runtime"),
            ("orchestration.yaml", "daemon-router", "orchestration"),
        ] {
            write_plugin(
                repo_dir.path(),
                filename,
                &format!(
                    "name: {name}\nkind: {kind}\nmode: daemon\nentry:\n  command: sleep\n  args: [\"30\"]\n"
                ),
            );
            write_plugin(
                repo_dir.path(),
                &format!("{kind}-oneshot.yaml"),
                &format!("name: {kind}-once\nkind: {kind}\nmode: one_shot\n"),
            );
            write_plugin(
                repo_dir.path(),
                &format!("{kind}-disabled.yaml"),
                &format!("name: {kind}-off\nkind: {kind}\nmode: daemon\nenabled: false\n"),
            );
        }

        let mut runtime =
            PluginRuntimeRegistry::load_dual_source(None, Some(repo_dir.path().to_path_buf()))
                .unwrap();
        for (kind, name) in [
            (PluginKind::Tool, "daemon-tool"),
            (PluginKind::Runtime, "daemon-runtime"),
            (PluginKind::Orchestration, "daemon-router"),
        ] {
            looprs_core::ports::test_contracts::assert_plugin_supervisor_contract(
                &mut runtime,
                kind,
                name,
            );
            looprs_core::ports::test_contracts::assert_plugin_supervisor_error_contract(
                &mut runtime,
                kind,
                &format!("{kind:?}-once").to_lowercase(),
                &format!("{kind:?}-off").to_lowercase(),
            );
        }
    }

    #[test]
    fn daemon_plugin_without_entry_is_rejected() {
        let repo_dir = TempDir::new().unwrap();
        write_plugin(
            repo_dir.path(),
            "daemon.yaml",
            r#"name: daemon-runtime
kind: runtime
mode: daemon
enabled: true"#,
        );

        let err =
            PluginRuntimeRegistry::load_dual_source(None, Some(repo_dir.path().to_path_buf()))
                .unwrap_err();
        assert!(err.to_string().contains("non-empty entry.command"));
    }

    #[test]
    fn blank_daemon_command_is_rejected() {
        let repo_dir = TempDir::new().unwrap();
        write_plugin(
            repo_dir.path(),
            "daemon.yaml",
            r#"name: blank-tool
kind: tool
mode: daemon
entry:
  command: "   ""#,
        );

        let err =
            PluginRuntimeRegistry::load_dual_source(None, Some(repo_dir.path().to_path_buf()))
                .unwrap_err();
        assert!(err.to_string().contains("non-empty entry.command"));
    }

    #[test]
    fn supervisor_errors_cover_unknown_oneshot_and_disabled_plugins() {
        let repo_dir = TempDir::new().unwrap();
        write_plugin(
            repo_dir.path(),
            "oneshot.yaml",
            "name: once\nkind: tool\nmode: one_shot\nentry:\n  command: true\n",
        );
        write_plugin(
            repo_dir.path(),
            "disabled.yaml",
            "name: off\nkind: runtime\nmode: daemon\nenabled: false\n",
        );

        let mut runtime =
            PluginRuntimeRegistry::load_dual_source(None, Some(repo_dir.path().to_path_buf()))
                .unwrap();

        let unknown = runtime
            .restart(PluginKind::Tool, "missing", "test")
            .unwrap_err();
        assert!(matches!(
            unknown,
            PluginSupervisorError::UnknownPlugin { .. }
        ));
        let oneshot = runtime
            .restart(PluginKind::Tool, "once", "test")
            .unwrap_err();
        assert!(matches!(oneshot, PluginSupervisorError::NotDaemon { .. }));
        let disabled = runtime
            .restart(PluginKind::Runtime, "off", "test")
            .unwrap_err();
        assert!(matches!(disabled, PluginSupervisorError::Disabled { .. }));
        let disabled_status = runtime.status(PluginKind::Runtime, "off").unwrap();
        assert_eq!(disabled_status.state, PluginHealthState::Disabled);
        assert!(disabled_status.pid.is_none());
    }

    #[test]
    fn unknown_plugin_kind_is_rejected() {
        let repo_dir = TempDir::new().unwrap();
        write_plugin(
            repo_dir.path(),
            "unknown.yaml",
            "name: mystery\nkind: unknown\nmode: one_shot\n",
        );

        let err = PluginManifestRegistry::load_from_directory(&repo_dir.path().to_path_buf())
            .unwrap_err();
        assert!(err.to_string().contains("unknown variant `unknown`"));
    }

    #[test]
    fn failed_health_probe_marks_daemon_unhealthy() {
        let repo_dir = TempDir::new().unwrap();
        write_plugin(
            repo_dir.path(),
            "daemon.yaml",
            r#"name: probed
kind: runtime
mode: daemon
entry:
  command: sleep
  args: ["30"]
  probe:
    command: false"#,
        );
        let mut runtime =
            PluginRuntimeRegistry::load_dual_source(None, Some(repo_dir.path().to_path_buf()))
                .unwrap();

        let status = runtime.probe(PluginKind::Runtime, "probed").unwrap();
        assert_eq!(status.state, PluginHealthState::Unhealthy);
        assert!(status.last_error.unwrap().contains("probe exited"));
    }

    #[test]
    fn probe_execution_error_marks_daemon_unhealthy() {
        let repo_dir = TempDir::new().unwrap();
        write_plugin(
            repo_dir.path(),
            "daemon.yaml",
            r#"name: missing-probe
kind: runtime
mode: daemon
entry:
  command: sleep
  args: ["30"]
  probe:
    command: definitely-missing-looprs-probe"#,
        );
        let mut runtime =
            PluginRuntimeRegistry::load_dual_source(None, Some(repo_dir.path().to_path_buf()))
                .unwrap();

        let error = runtime
            .probe(PluginKind::Runtime, "missing-probe")
            .unwrap_err();
        assert!(matches!(error, PluginSupervisorError::ProbeFailed { .. }));
        let status = runtime
            .status(PluginKind::Runtime, "missing-probe")
            .unwrap();
        assert_eq!(status.state, PluginHealthState::Unhealthy);
        assert!(status.last_error.unwrap().contains("was not found"));
    }

    #[test]
    fn restart_limit_prevents_unbounded_relaunches() {
        let repo_dir = TempDir::new().unwrap();
        write_plugin(
            repo_dir.path(),
            "daemon.yaml",
            "name: bounded\nkind: tool\nmode: daemon\nentry:\n  command: sleep\n  args: [\"30\"]\n",
        );
        let mut runtime =
            PluginRuntimeRegistry::load_dual_source(None, Some(repo_dir.path().to_path_buf()))
                .unwrap();

        for _ in 0..MAX_PLUGIN_RESTARTS {
            runtime
                .restart(PluginKind::Tool, "bounded", "test")
                .unwrap();
        }
        let err = runtime
            .restart(PluginKind::Tool, "bounded", "saturated")
            .unwrap_err();
        assert!(matches!(
            err,
            PluginSupervisorError::RestartLimitReached { .. }
        ));
    }

    #[test]
    fn failed_refresh_preserves_running_registry() {
        let repo_dir = TempDir::new().unwrap();
        write_plugin(
            repo_dir.path(),
            "daemon.yaml",
            "name: stable\nkind: runtime\nmode: daemon\nentry:\n  command: sleep\n  args: [\"30\"]\n",
        );
        let mut runtime =
            PluginRuntimeRegistry::load_dual_source(None, Some(repo_dir.path().to_path_buf()))
                .unwrap();
        let original_pid = runtime.status(PluginKind::Runtime, "stable").unwrap().pid;

        write_plugin(repo_dir.path(), "broken.yaml", "name: [not valid");
        assert!(runtime.refresh_if_changed().is_err());
        let status = runtime.status(PluginKind::Runtime, "stable").unwrap();

        assert_eq!(status.state, PluginHealthState::Healthy);
        assert_eq!(status.pid, original_pid);
    }

    #[test]
    fn failed_cross_kind_refresh_is_atomic() {
        let repo_dir = TempDir::new().unwrap();
        write_plugin(
            repo_dir.path(),
            "tool.yaml",
            "name: stable-tool\nkind: tool\nmode: daemon\nentry:\n  command: sleep\n  args: [\"30\"]\n",
        );
        let mut runtime =
            PluginRuntimeRegistry::load_dual_source(None, Some(repo_dir.path().to_path_buf()))
                .unwrap();
        let original_pid = runtime.status(PluginKind::Tool, "stable-tool").unwrap().pid;

        write_plugin(
            repo_dir.path(),
            "tool.yaml",
            "name: stable-tool\nkind: tool\nmode: daemon\ndescription: changed\nentry:\n  command: sleep\n  args: [\"30\"]\n",
        );
        write_plugin(
            repo_dir.path(),
            "runtime.yaml",
            "name: broken-runtime\nkind: runtime\nmode: daemon\nrequired: true\nentry:\n  command: definitely-missing-looprs-plugin\n",
        );

        assert!(runtime.refresh_if_changed().is_err());
        let status = runtime.status(PluginKind::Tool, "stable-tool").unwrap();
        assert_eq!(status.pid, original_pid);
        assert_eq!(status.restart_count, 0);
    }

    #[test]
    fn single_word_trigger_uses_token_boundaries() {
        let repo_dir = TempDir::new().unwrap();
        write_plugin(
            repo_dir.path(),
            "route.yaml",
            r#"name: route
kind: orchestration
triggers: ["gate"]
route_to_agent: taskit"#,
        );

        let mut runtime =
            PluginRuntimeRegistry::load_dual_source(None, Some(repo_dir.path().to_path_buf()))
                .unwrap();

        let no_match = runtime
            .select_agent_for_prompt("please investigate this failure")
            .unwrap();
        assert!(no_match.is_none());

        let yes_match = runtime
            .select_agent_for_prompt("taskit health --gate now")
            .unwrap();
        assert_eq!(yes_match.unwrap().agent_name, "taskit");
    }

    #[test]
    fn bundled_plugin_manifests_are_valid_oneshot_routes() {
        let plugin_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(".looprs/plugins");
        let registry = PluginManifestRegistry::load_from_directory(&plugin_dir).unwrap();

        for name in ["taskit-orchestration", "opencode-orchestration"] {
            let manifest = registry.get(PluginKind::Orchestration, name).unwrap();
            assert_eq!(manifest.mode, PluginExecutionMode::OneShot);
            assert!(manifest.entry.is_none());
            assert!(manifest.route_to_agent.is_some());
        }
    }

    fn protocol_entry(script: &str, timeout_ms: u64) -> PluginEntry {
        PluginEntry {
            command: "sh".to_string(),
            args: vec!["-c".to_string(), script.to_string()],
            probe: None,
            protocol: PluginProtocol::JsonLines,
            timeout_ms,
        }
    }

    fn tool_manifest(name: &str, mode: PluginExecutionMode, script: &str) -> PluginManifest {
        PluginManifest {
            name: name.to_string(),
            kind: PluginKind::Tool,
            description: None,
            enabled: true,
            required: true,
            mode,
            entry: Some(protocol_entry(script, 500)),
            triggers: Vec::new(),
            route_to_agent: None,
            tools: vec![PluginToolManifest {
                name: format!("{name}_echo"),
                description: "Echo through a manifest plugin".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {"text": {"type": "string"}}
                }),
            }],
        }
    }

    #[test]
    fn manifest_tool_registers_schema_and_executes_oneshot_request() {
        let script = r#"read line
printf '%s\n' '{"type":"response","id":"1","result":{"text":"plugin-output"}}'"#;
        let mut runtime = PluginRuntimeRegistry::default();
        runtime.registry.register(tool_manifest(
            "oneshot",
            PluginExecutionMode::OneShot,
            script,
        ));
        let rules = crate::rules::RuleRegistry::new();

        let definitions = runtime.tool_definitions().unwrap();
        let output = runtime
            .execute_tool(
                "oneshot_echo",
                &serde_json::json!({"text": "hello"}),
                &rules,
                &PluginCallOptions::default(),
            )
            .unwrap();

        assert_eq!(definitions.len(), 1);
        assert_eq!(definitions[0].name, "oneshot_echo");
        assert_eq!(output, serde_json::json!({"text": "plugin-output"}));
    }

    #[test]
    fn plugin_execution_is_denied_before_process_side_effects() {
        use crate::rules::{
            ExecutionBoundary, ExecutionPolicy, PolicyEffect, PolicySource, RuleRegistry,
        };

        let dir = TempDir::new().unwrap();
        let marker = dir.path().join("executed");
        let script = format!(
            "touch '{}'; read line; printf '%s\\n' '{{\"type\":\"response\",\"id\":\"1\",\"result\":null}}'",
            marker.display()
        );
        let mut runtime = PluginRuntimeRegistry::default();
        runtime.registry.register(tool_manifest(
            "governed",
            PluginExecutionMode::OneShot,
            &script,
        ));
        let mut rules = RuleRegistry::new();
        rules.register_policy(ExecutionPolicy {
            id: "deny-plugin".to_string(),
            effect: PolicyEffect::Deny,
            boundary: ExecutionBoundary::Plugin,
            target: "governed_echo".to_string(),
            input_contains: None,
            reason: "plugin disabled".to_string(),
            audit: Default::default(),
            source: PolicySource::Repository,
        });

        let error = runtime
            .execute_tool(
                "governed_echo",
                &serde_json::json!({}),
                &rules,
                &PluginCallOptions::default(),
            )
            .unwrap_err();

        assert!(error.to_string().contains("plugin disabled"));
        assert!(!marker.exists());
    }

    #[test]
    fn daemon_protocol_reports_readiness_executes_restarts_and_shuts_down() {
        let script = r#"printf '%s\n' '{"type":"ready"}'
while IFS= read -r line; do
  printf '%s\n' '{"type":"response","id":"1","result":"daemon-output"}'
done"#;
        let dir = TempDir::new().unwrap();
        let manifest = tool_manifest("daemon", PluginExecutionMode::Daemon, script);
        write_plugin(
            dir.path(),
            "daemon.yaml",
            &serde_yaml::to_string(&manifest).unwrap(),
        );
        let mut runtime =
            PluginRuntimeRegistry::load_dual_source(None, Some(dir.path().to_path_buf())).unwrap();
        let rules = crate::rules::RuleRegistry::new();

        let initial = runtime.probe(PluginKind::Tool, "daemon").unwrap();
        let output = runtime
            .execute_tool(
                "daemon_echo",
                &serde_json::json!({}),
                &rules,
                &PluginCallOptions::default(),
            )
            .unwrap();
        runtime
            .restart(PluginKind::Tool, "daemon", "test restart")
            .unwrap();
        let restarted = runtime.status(PluginKind::Tool, "daemon").unwrap();
        runtime.shutdown(PluginKind::Tool, "daemon").unwrap();
        let stopped = runtime.status(PluginKind::Tool, "daemon").unwrap();

        assert_eq!(initial.state, PluginHealthState::Healthy);
        assert_eq!(output, serde_json::json!("daemon-output"));
        assert_ne!(initial.pid, restarted.pid);
        assert_eq!(restarted.restart_count, 1);
        assert_eq!(stopped.state, PluginHealthState::Stopped);
        assert!(stopped.pid.is_none());
    }

    #[test]
    fn required_plugin_fails_closed_but_optional_plugin_records_diagnostic() {
        let dir = TempDir::new().unwrap();
        let mut required = tool_manifest(
            "required",
            PluginExecutionMode::Daemon,
            "printf '%s\\n' 'not-json'; sleep 30",
        );
        required.required = true;
        write_plugin(
            dir.path(),
            "required.yaml",
            &serde_yaml::to_string(&required).unwrap(),
        );
        assert!(
            PluginRuntimeRegistry::load_dual_source(None, Some(dir.path().to_path_buf())).is_err()
        );

        std::fs::remove_file(dir.path().join("required.yaml")).unwrap();
        let mut optional = required;
        optional.name = "optional".to_string();
        optional.required = false;
        optional.tools[0].name = "optional_echo".to_string();
        write_plugin(
            dir.path(),
            "optional.yaml",
            &serde_yaml::to_string(&optional).unwrap(),
        );
        let mut runtime =
            PluginRuntimeRegistry::load_dual_source(None, Some(dir.path().to_path_buf())).unwrap();

        let status = runtime.status(PluginKind::Tool, "optional").unwrap();
        assert_eq!(status.state, PluginHealthState::Unhealthy);
        assert!(
            runtime
                .diagnostics()
                .iter()
                .any(|message| { message.contains("optional") && message.contains("readiness") })
        );
    }

    #[test]
    fn oneshot_timeout_captures_stderr_and_cancellation_prevents_launch() {
        let mut runtime = PluginRuntimeRegistry::default();
        runtime.registry.register(tool_manifest(
            "slow",
            PluginExecutionMode::OneShot,
            "read line; printf 'starting' >&2; exec sleep 2",
        ));
        let rules = crate::rules::RuleRegistry::new();
        let options = PluginCallOptions::with_timeout(std::time::Duration::from_millis(30));

        let timeout_error = runtime
            .execute_tool("slow_echo", &serde_json::json!({}), &rules, &options)
            .unwrap_err();
        assert!(timeout_error.to_string().contains("timed out"));
        assert!(timeout_error.to_string().contains("starting"));

        let dir = TempDir::new().unwrap();
        let marker = dir.path().join("cancelled-launch");
        let script = format!("touch '{}'; read line", marker.display());
        runtime.registry.register(tool_manifest(
            "cancelled",
            PluginExecutionMode::OneShot,
            &script,
        ));
        let cancellation = PluginCancellation::new();
        cancellation.cancel();
        let cancelled = PluginCallOptions::default().with_cancellation(cancellation);
        let error = runtime
            .execute_tool("cancelled_echo", &serde_json::json!({}), &rules, &cancelled)
            .unwrap_err();
        assert!(error.to_string().contains("cancelled"));
        assert!(!marker.exists());
    }

    #[test]
    fn runtime_plugins_execute_in_oneshot_and_daemon_modes() {
        let oneshot_script = r#"read line
printf '%s\n' '{"type":"response","id":"1","result":"once"}'"#;
        let daemon_script = r#"printf '%s\n' '{"type":"ready"}'
while IFS= read -r line; do
  printf '%s\n' '{"type":"response","id":"1","result":"resident"}'
done"#;
        let dir = TempDir::new().unwrap();
        for (name, mode, script) in [
            ("once-runtime", PluginExecutionMode::OneShot, oneshot_script),
            ("daemon-runtime", PluginExecutionMode::Daemon, daemon_script),
        ] {
            let manifest = PluginManifest {
                name: name.to_string(),
                kind: PluginKind::Runtime,
                description: None,
                enabled: true,
                required: true,
                mode,
                entry: Some(protocol_entry(script, 500)),
                triggers: Vec::new(),
                route_to_agent: None,
                tools: Vec::new(),
            };
            write_plugin(
                dir.path(),
                &format!("{name}.yaml"),
                &serde_yaml::to_string(&manifest).unwrap(),
            );
        }
        let mut runtime =
            PluginRuntimeRegistry::load_dual_source(None, Some(dir.path().to_path_buf())).unwrap();
        let rules = crate::rules::RuleRegistry::new();

        let once = runtime
            .execute_runtime(
                "once-runtime",
                &serde_json::json!({"work": 1}),
                &rules,
                &PluginCallOptions::default(),
            )
            .unwrap();
        let resident = runtime
            .execute_runtime(
                "daemon-runtime",
                &serde_json::json!({"work": 2}),
                &rules,
                &PluginCallOptions::default(),
            )
            .unwrap();

        assert_eq!(once, serde_json::json!("once"));
        assert_eq!(resident, serde_json::json!("resident"));
        assert_eq!(
            runtime
                .status(PluginKind::Runtime, "daemon-runtime")
                .unwrap()
                .state,
            PluginHealthState::Healthy
        );
    }

    #[tokio::test]
    async fn manifest_tool_runtime_registers_with_agent_tool_ports() {
        use crate::tools::{
            DefaultToolExecutor, StaticToolCatalog, ToolCatalog, ToolContext, ToolDispatcher,
        };

        let script = r#"read line
printf '%s\n' '{"type":"response","id":"1","result":"adapter-output"}'"#;
        let mut registry = PluginRuntimeRegistry::default();
        registry.registry.register(tool_manifest(
            "adapter",
            PluginExecutionMode::OneShot,
            script,
        ));
        let adapter = ManifestToolRuntime::new(
            Arc::new(Mutex::new(registry)),
            Arc::new(crate::rules::RuleRegistry::new()),
            Arc::new(StaticToolCatalog::default()),
            Arc::new(DefaultToolExecutor),
        );
        let context = ToolContext::from_working_dir(
            std::env::current_dir().unwrap(),
            crate::fs_mode::FsMode::Write,
        );

        let definitions = ToolCatalog::definitions(&adapter).await.unwrap();
        let output =
            ToolDispatcher::execute(&adapter, "adapter_echo", &serde_json::json!({}), &context)
                .await
                .unwrap();

        assert_eq!(definitions[0].name, "adapter_echo");
        assert_eq!(output, "\"adapter-output\"");
    }
}
