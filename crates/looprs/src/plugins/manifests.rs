use looprs_core::ports::{
    OrchestrationPluginPort, PluginAgentSelection, PluginExecutionMode, PluginHealthState,
    PluginKind, PluginSupervisorStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginEntry {
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    pub name: String,
    pub kind: PluginKind,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub mode: PluginExecutionMode,
    #[serde(default)]
    pub entry: Option<PluginEntry>,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
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
    pub fn new() -> Self {
        Self {
            manifests: HashMap::new(),
        }
    }

    pub fn register(&mut self, manifest: PluginManifest) {
        self.manifests
            .insert((manifest.kind, manifest.name.clone()), manifest);
    }

    pub fn get(&self, kind: PluginKind, name: &str) -> Option<&PluginManifest> {
        self.manifests.get(&(kind, name.to_string()))
    }

    pub fn list_by_kind(&self, kind: PluginKind) -> Vec<&PluginManifest> {
        let mut items: Vec<&PluginManifest> = self
            .manifests
            .iter()
            .filter_map(|((k, _), v)| (*k == kind).then_some(v))
            .collect();
        items.sort_by_key(|m| &m.name);
        items
    }

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

            match Self::parse_manifest(&path) {
                Ok(manifest) => registry.register(manifest),
                Err(e) => crate::ui::warn(format!(
                    "Warning: Failed to load plugin {}: {}",
                    path.display(),
                    e
                )),
            }
        }

        Ok(registry)
    }

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

#[derive(Debug, Clone, Default)]
struct KindSupervisor {
    statuses: HashMap<String, PluginSupervisorStatus>,
}

impl KindSupervisor {
    fn reconcile(
        &mut self,
        kind: PluginKind,
        old: &PluginManifestRegistry,
        new: &PluginManifestRegistry,
    ) {
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

        self.statuses
            .retain(|name, _| new_map.contains_key(name.as_str()));

        for (name, manifest) in new_map {
            if manifest.mode != PluginExecutionMode::Daemon {
                self.statuses.remove(name);
                continue;
            }

            let previous = old_map.get(name);
            let state = if !manifest.enabled {
                PluginHealthState::Disabled
            } else {
                PluginHealthState::Healthy
            };

            let restart_count = match (self.statuses.get(name), previous) {
                (Some(status), Some(old_manifest)) if *old_manifest != manifest => {
                    status.restart_count.saturating_add(1)
                }
                (Some(status), Some(_)) => status.restart_count,
                (Some(status), None) => status.restart_count,
                (None, _) => 0,
            };

            self.statuses.insert(
                name.to_string(),
                PluginSupervisorStatus {
                    plugin_name: name.to_string(),
                    kind,
                    state,
                    restart_count,
                },
            );
        }
    }
}

#[derive(Debug, Clone, Default)]
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

    pub fn list_orchestration_plugins(&self) -> Vec<&PluginManifest> {
        self.registry.list_by_kind(PluginKind::Orchestration)
    }

    pub fn orchestration_plugin(&self, name: &str) -> Option<&PluginManifest> {
        self.registry.get(PluginKind::Orchestration, name)
    }

    pub fn status_for_kind(&self, kind: PluginKind, name: &str) -> Option<&PluginSupervisorStatus> {
        match kind {
            PluginKind::Tool => self.tool_supervisor.statuses.get(name),
            PluginKind::Runtime => self.runtime_supervisor.statuses.get(name),
            PluginKind::Orchestration => self.orchestration_supervisor.statuses.get(name),
        }
    }

    pub fn refresh_if_changed(&mut self) -> anyhow::Result<bool> {
        let fingerprint = self.compute_fingerprint()?;
        if self.fingerprint.as_ref() == Some(&fingerprint) {
            return Ok(false);
        }
        self.reload_now()?;
        Ok(true)
    }

    fn reload_now(&mut self) -> anyhow::Result<()> {
        let old = self.registry.clone();
        self.registry = PluginManifestRegistry::load_dual_source(
            self.user_dir.as_ref(),
            self.repo_dir.as_ref(),
        )?;

        self.tool_supervisor
            .reconcile(PluginKind::Tool, &old, &self.registry);
        self.runtime_supervisor
            .reconcile(PluginKind::Runtime, &old, &self.registry);
        self.orchestration_supervisor
            .reconcile(PluginKind::Orchestration, &old, &self.registry);

        self.fingerprint = Some(self.compute_fingerprint()?);
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
        for manifest in self.registry.list_by_kind(PluginKind::Orchestration) {
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

#[cfg(test)]
mod tests {
    use super::*;
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
    fn daemon_plugins_record_supervisor_status() {
        let repo_dir = TempDir::new().unwrap();
        write_plugin(
            repo_dir.path(),
            "daemon.yaml",
            r#"name: daemon-router
kind: orchestration
mode: daemon
enabled: true
triggers: ["route"]
route_to_agent: planner"#,
        );

        let runtime =
            PluginRuntimeRegistry::load_dual_source(None, Some(repo_dir.path().to_path_buf()))
                .unwrap();
        let status = runtime
            .status_for_kind(PluginKind::Orchestration, "daemon-router")
            .unwrap();

        assert_eq!(status.state, PluginHealthState::Healthy);
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
}
