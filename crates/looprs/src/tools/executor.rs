use serde_json::Value;

use crate::tools::{ToolContext, ToolError, execute_tool};

/// Port: dispatch a named agent tool call.
///
/// Abstracts the free `execute_tool` function so the Agent can be tested
/// with a stub executor instead of a real subprocess/filesystem backend.
pub trait ToolExecutor: Send + Sync {
    fn execute(&self, name: &str, args: &Value, ctx: &ToolContext) -> Result<String, ToolError>;
}

/// Production adapter: delegates to `tools::execute_tool`.
pub struct DefaultToolExecutor;

impl ToolExecutor for DefaultToolExecutor {
    fn execute(&self, name: &str, args: &Value, ctx: &ToolContext) -> Result<String, ToolError> {
        execute_tool(name, args, ctx)
    }
}

/// Test stub: always succeeds with a fixed response.
#[cfg(test)]
pub struct StubToolExecutor {
    pub response: String,
}

#[cfg(test)]
impl Default for StubToolExecutor {
    fn default() -> Self {
        Self {
            response: "ok".to_string(),
        }
    }
}

#[cfg(test)]
impl ToolExecutor for StubToolExecutor {
    fn execute(&self, _name: &str, _args: &Value, _ctx: &ToolContext) -> Result<String, ToolError> {
        Ok(self.response.clone())
    }
}
