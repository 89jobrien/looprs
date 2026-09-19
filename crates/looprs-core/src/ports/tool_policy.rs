//! Typed policy for tools available to a delegated agent turn.

use std::collections::HashSet;

use crate::api::ToolDefinition;

/// Default-deny allowlist applied to delegated tool advertisement and execution.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DelegatedToolPolicy {
    allowed: HashSet<String>,
    ordered: Vec<String>,
}

impl DelegatedToolPolicy {
    /// Build a policy from typed tool names.
    pub fn from_names(names: impl IntoIterator<Item = impl Into<String>>) -> Self {
        let mut allowed = HashSet::new();
        let mut ordered = Vec::new();
        for name in names {
            let name = name.into();
            let name = name.trim();
            if !name.is_empty() && allowed.insert(name.to_string()) {
                ordered.push(name.to_string());
            }
        }
        Self { allowed, ordered }
    }

    /// Parse a comma-separated allowlist. Missing, empty, and malformed lists deny all tools.
    pub fn from_csv(raw: Option<&str>) -> Self {
        Self::from_names(raw.into_iter().flat_map(|value| value.split(',')))
    }

    /// Return whether the delegated turn may execute `tool_name`.
    pub fn allows(&self, tool_name: &str) -> bool {
        self.allowed.contains(tool_name)
    }

    /// Remove definitions that this delegated turn may not execute.
    pub fn filter_definitions(&self, definitions: &mut Vec<ToolDefinition>) {
        definitions.retain(|definition| self.allows(&definition.name));
    }

    /// Return allowed names in stable order for compatibility metadata.
    pub fn names(&self) -> Vec<&str> {
        self.ordered.iter().map(String::as_str).collect()
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
        assert_eq!(policy.names(), vec!["read", "grep"]);
    }
}
