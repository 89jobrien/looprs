//! Typed policy for tools available to a delegated agent turn.

use std::collections::HashSet;

use crate::api::ToolDefinition;

/// Default-deny allowlist applied to delegated tool advertisement and execution.
#[derive(Debug, Clone, Default)]
pub struct DelegatedToolPolicy {
    allowed: HashSet<String>,
}

impl DelegatedToolPolicy {
    /// Parse a comma-separated allowlist. Missing, empty, and malformed lists deny all tools.
    pub fn from_csv(raw: Option<&str>) -> Self {
        let allowed = raw
            .into_iter()
            .flat_map(|value| value.split(','))
            .map(str::trim)
            .filter(|tool| !tool.is_empty())
            .map(ToOwned::to_owned)
            .collect();
        Self { allowed }
    }

    /// Return whether the delegated turn may execute `tool_name`.
    pub fn allows(&self, tool_name: &str) -> bool {
        self.allowed.contains(tool_name)
    }

    /// Remove definitions that this delegated turn may not execute.
    pub fn filter_definitions(&self, definitions: &mut Vec<ToolDefinition>) {
        definitions.retain(|definition| self.allows(&definition.name));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_empty_delegated_allowlist_denies_every_tool() {
        assert!(!DelegatedToolPolicy::from_csv(None).allows("read"));
        assert!(!DelegatedToolPolicy::from_csv(Some(" , ")).allows("read"));
    }

    #[test]
    fn delegated_allowlist_trims_and_deduplicates_names() {
        let policy = DelegatedToolPolicy::from_csv(Some(" read, grep,read "));
        assert!(policy.allows("read"));
        assert!(policy.allows("grep"));
        assert!(!policy.allows("bash"));
    }
}
