use anyhow::Result;
use std::collections::HashMap;

use looprs::AgentRegistry;
use looprs::app_config::AppConfig;
use looprs::plugins::manifests::PluginRuntimeRegistry;
use looprs::ports::OrchestrationPluginPort;
use looprs::ui;

pub(crate) fn prepare_user_prompt(
    raw_prompt: &str,
    app_config: &AppConfig,
    agent_registry: &AgentRegistry,
    plugin_runtime: &mut PluginRuntimeRegistry,
) -> Result<(String, HashMap<String, String>, Option<String>)> {
    if agent_registry.is_empty() {
        return Ok((raw_prompt.to_string(), HashMap::new(), None));
    }

    let explicit = parse_explicit_agent_tag(raw_prompt);
    let (selection, task_prompt, selection_mode, routed_by_plugin) = match explicit {
        Some((agent_name, remainder)) => {
            if let Some(agent) = agent_registry.get(agent_name) {
                (Some(agent), remainder, "explicit", None)
            } else {
                ui::warn(format!(
                    "Unknown explicit agent tag '#{agent_name}'; falling back to auto selection"
                ));
                let fallback_prompt = if remainder.is_empty() {
                    raw_prompt
                } else {
                    remainder
                };
                (
                    agent_registry.select_for_prompt(
                        fallback_prompt,
                        app_config.agents.default_agent.as_deref(),
                        app_config.agents.delegate_by_default,
                    ),
                    fallback_prompt,
                    "auto",
                    None,
                )
            }
        }
        None => match plugin_runtime.select_agent_for_prompt(raw_prompt)? {
            Some(plugin_selection) => {
                let manifest = plugin_runtime
                    .orchestration_plugin(&plugin_selection.plugin_name)
                    .cloned();

                if let Some(agent) = agent_registry.get(&plugin_selection.agent_name) {
                    (
                        Some(agent),
                        raw_prompt,
                        "plugin",
                        Some(plugin_selection.plugin_name),
                    )
                } else if manifest.as_ref().is_some_and(|m| m.required) {
                    anyhow::bail!(
                        "Required orchestration plugin '{}' routed to unknown agent '{}'",
                        plugin_selection.plugin_name,
                        plugin_selection.agent_name
                    );
                } else {
                    (
                        agent_registry.select_for_prompt(
                            raw_prompt,
                            app_config.agents.default_agent.as_deref(),
                            app_config.agents.delegate_by_default,
                        ),
                        raw_prompt,
                        "auto",
                        None,
                    )
                }
            }
            None => (
                agent_registry.select_for_prompt(
                    raw_prompt,
                    app_config.agents.default_agent.as_deref(),
                    app_config.agents.delegate_by_default,
                ),
                raw_prompt,
                "auto",
                None,
            ),
        },
    };

    let Some(agent) = selection else {
        return Ok((raw_prompt.to_string(), HashMap::new(), None));
    };

    let mut metadata = HashMap::new();
    metadata.insert("orchestration.mode".to_string(), "delegated".to_string());
    metadata.insert("orchestration.agent".to_string(), agent.name.clone());
    metadata.insert(
        "orchestration.strategy".to_string(),
        app_config.agents.orchestration.clone(),
    );
    metadata.insert(
        "orchestration.selection".to_string(),
        selection_mode.to_string(),
    );
    if let Some(plugin_name) = routed_by_plugin {
        metadata.insert("orchestration.plugin".to_string(), plugin_name);
    }

    // TODO(feature-idea-5): Resolve `agent.skills` into delegated context and
    // enforce `agent.tools` when defining and executing tools for this turn.
    let role = agent
        .role
        .clone()
        .unwrap_or_else(|| "Specialized assistant".to_string());
    let description = agent.description.clone().unwrap_or_default();
    let system_prompt = agent.system_prompt.clone().unwrap_or_default();
    let constraints = if agent.constraints.is_empty() {
        String::new()
    } else {
        agent
            .constraints
            .iter()
            .map(|c| format!("- {c}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let rewritten = format!(
        "[Delegation]\nAgent: {}\nRole: {}\nDescription: {}\nSystem Prompt:\n{}\nConstraints:\n{}\n\nTask:\n{}",
        agent.name, role, description, system_prompt, constraints, task_prompt
    );

    Ok((rewritten, metadata, Some(agent.name.clone())))
}

fn parse_explicit_agent_tag(raw_prompt: &str) -> Option<(&str, &str)> {
    let trimmed = raw_prompt.trim_start();
    if !trimmed.starts_with('#') {
        return None;
    }

    let after_hash = &trimmed[1..];
    if after_hash.is_empty() {
        return None;
    }

    let split_at = after_hash
        .char_indices()
        .find_map(|(idx, ch)| ch.is_whitespace().then_some(idx))
        .unwrap_or(after_hash.len());

    let agent_name = &after_hash[..split_at];
    if agent_name.is_empty()
        || !agent_name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return None;
    }

    let remainder = after_hash[split_at..].trim_start();
    Some((agent_name, remainder))
}

#[cfg(test)]
mod tests {
    use super::parse_explicit_agent_tag;

    #[test]
    fn parses_hash_agent_tag_with_prompt() {
        let parsed = parse_explicit_agent_tag("#taskit investigate regression").unwrap();
        assert_eq!(parsed.0, "taskit");
        assert_eq!(parsed.1, "investigate regression");
    }

    #[test]
    fn parses_hash_agent_tag_without_prompt() {
        let parsed = parse_explicit_agent_tag("#opencode").unwrap();
        assert_eq!(parsed.0, "opencode");
        assert_eq!(parsed.1, "");
    }

    #[test]
    fn rejects_invalid_hash_agent_tag() {
        assert!(parse_explicit_agent_tag("#taskit/alpha do thing").is_none());
        assert!(parse_explicit_agent_tag("not a tag").is_none());
    }
}
