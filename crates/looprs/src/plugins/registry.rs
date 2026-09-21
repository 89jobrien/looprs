//! Caches executable-path lookups performed by pluggable tool resolvers.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Resolves a tool name to its executable path. Implementations decide
/// the resolution strategy (e.g. searching `PATH`, as
/// [`crate::plugins::PathResolver`] does).
pub trait ToolResolver: Send + Sync {
    /// Returns the executable path for `tool`, or `None` if it cannot be
    /// found.
    fn resolve(&self, tool: &str) -> Option<PathBuf>;
}

/// Caches tool resolution results from a [`ToolResolver`] so repeated
/// lookups for the same tool name avoid re-running the resolver's
/// underlying strategy (e.g. a `PATH` scan).
pub struct ToolRegistry {
    resolver: Arc<dyn ToolResolver>,
    cache: Mutex<HashMap<String, Option<PathBuf>>>,
}

impl ToolRegistry {
    /// Creates a registry backed by `resolver`, with an empty cache.
    pub fn new(resolver: Arc<dyn ToolResolver>) -> Self {
        Self {
            resolver,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Returns whether `tool` resolves to an executable. Caches the
    /// result for subsequent lookups.
    pub fn has(&self, tool: &str) -> bool {
        self.resolve(tool).is_some()
    }

    /// Resolves `tool`, caching the result for subsequent lookups.
    ///
    /// # Errors
    /// Returns an [`std::io::ErrorKind::NotFound`] error if `tool` cannot
    /// be resolved.
    pub fn require(&self, tool: &str) -> std::io::Result<PathBuf> {
        self.resolve(tool).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("tool not found in PATH: {tool}"),
            )
        })
    }

    fn resolve(&self, tool: &str) -> Option<PathBuf> {
        let mut cache = self.cache.lock().unwrap();
        if let Some(v) = cache.get(tool) {
            return v.clone();
        }

        let resolved = self.resolver.resolve(tool);
        cache.insert(tool.to_string(), resolved.clone());
        resolved
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct StaticResolver {
        map: HashMap<String, Option<PathBuf>>,
    }

    impl ToolResolver for StaticResolver {
        fn resolve(&self, tool: &str) -> Option<PathBuf> {
            self.map.get(tool).cloned().unwrap_or(None)
        }
    }

    #[test]
    fn registry_caches_results() {
        let mut map = HashMap::new();
        map.insert("a".to_string(), Some(PathBuf::from("/bin/a")));

        let reg = ToolRegistry::new(Arc::new(StaticResolver { map }));
        assert!(reg.has("a"));
        assert!(reg.has("a"));
        assert!(!reg.has("missing"));
    }

    #[test]
    fn require_errors_when_missing() {
        let reg = ToolRegistry::new(Arc::new(StaticResolver {
            map: HashMap::new(),
        }));
        let err = reg.require("nope").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }
}
