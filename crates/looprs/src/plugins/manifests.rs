use looprs_core::ports::{
    OrchestrationPluginPort, OrchestrationSupervisorPort, PluginAgentSelection,
    PluginExecutionMode, PluginHealthState, PluginKind, PluginSupervisorError,
    PluginSupervisorPort, PluginSupervisorStatus, RuntimeSupervisorPort, ToolSupervisorPort,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

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

#[derive(Debug)]
struct ManagedPlugin {
    status: PluginSupervisorStatus,
    child: Option<Child>,
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
        Ok(())
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
        let child = Command::new(program)
            .args(&entry.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| PluginSupervisorError::LaunchFailed {
                kind,
                plugin_name: manifest.name.clone(),
                message: error.to_string(),
            })?;
        let pid = child.id();
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
            replacements.push((
                (*name).to_string(),
                Self::launch(
                    kind,
                    manifest,
                    restart_count,
                    (restart_count > 0).then(|| "manifest changed".to_string()),
                )?,
            ));
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

    fn supervisor_mut(&mut self, kind: PluginKind) -> &mut KindSupervisor {
        match kind {
            PluginKind::Tool => &mut self.tool_supervisor,
            PluginKind::Runtime => &mut self.runtime_supervisor,
            PluginKind::Orchestration => &mut self.orchestration_supervisor,
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
            "name: broken-runtime\nkind: runtime\nmode: daemon\nentry:\n  command: definitely-missing-looprs-plugin\n",
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
}
