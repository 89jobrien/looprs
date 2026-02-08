use super::{Action, Hook};
use crate::events::EventContext;
use std::process::Command;

pub struct HookExecutor;

#[derive(Debug, Clone)]
pub struct HookResult {
    pub hook_name: String,
    pub action_index: usize,
    pub output: String,
    pub inject_key: Option<String>,
}

impl HookExecutor {
    /// Execute a single hook and collect results
    pub fn execute_hook(hook: &Hook, context: &EventContext) -> anyhow::Result<Vec<HookResult>> {
        let mut results = Vec::new();

        // Check condition if present
        if let Some(condition) = &hook.condition {
            if !Self::eval_condition(condition, context)? {
                return Ok(results); // Skip hook if condition fails
            }
        }

        // Execute each action
        for (idx, action) in hook.actions.iter().enumerate() {
            if let Some(result) = Self::execute_action(action, context)? {
                results.push(HookResult {
                    hook_name: hook.name.clone(),
                    action_index: idx,
                    output: result.0,
                    inject_key: result.1,
                });
            }
        }

        Ok(results)
    }

    /// Execute a single action and return (output, inject_key)
    fn execute_action(
        action: &Action,
        context: &EventContext,
    ) -> anyhow::Result<Option<(String, Option<String>)>> {
        match action {
            Action::Command { command, inject_as } => {
                let output = Self::run_command(command)?;
                Ok(Some((output, inject_as.clone())))
            }
            Action::Message { text } => Ok(Some((text.clone(), None))),
            Action::Conditional {
                condition,
                then: actions,
            } => {
                if Self::eval_condition(condition, context)? {
                    for action in actions {
                        Self::execute_action(action, context)?;
                    }
                }
                Ok(None)
            }
        }
    }

    /// Run a shell command and capture output
    fn run_command(command_str: &str) -> anyhow::Result<String> {
        let output = Command::new("sh").arg("-c").arg(command_str).output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("Hook command failed: {}", stderr);
        }

        Ok(stdout.trim().to_string())
    }

    /// Evaluate simple conditions (very basic for now)
    fn eval_condition(condition: &str, _context: &EventContext) -> anyhow::Result<bool> {
        // Simple condition evaluation: "on_branch:main" or "has_tool:git"
        if condition.starts_with("on_branch:") {
            let branch = condition.strip_prefix("on_branch:").unwrap_or("");
            // Would check actual branch here
            return Ok(branch == "main" || branch == "*"); // For now, accept main or wildcard
        }

        if condition.starts_with("has_tool:") {
            let tool = condition.strip_prefix("has_tool:").unwrap_or("");
            return Ok(Self::check_tool_available(tool)?);
        }

        // If we don't recognize the condition, assume it passes (graceful degradation)
        Ok(true)
    }

    /// Check if a tool is available in PATH
    fn check_tool_available(tool: &str) -> anyhow::Result<bool> {
        let output = Command::new("sh")
            .arg("-c")
            .arg(&format!("which {} 2>/dev/null", tool))
            .output()?;

        Ok(output.status.success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_command_success() {
        let output = HookExecutor::run_command("echo hello").unwrap();
        assert_eq!(output, "hello");
    }

    #[test]
    fn test_run_command_with_pipes() {
        let output = HookExecutor::run_command("echo -e 'a\\nb\\nc' | wc -l").unwrap();
        let lines: i32 = output.trim().parse().unwrap_or(0);
        assert!(lines > 0);
    }

    #[test]
    fn test_condition_on_branch() {
        let context = EventContext::new();
        assert!(HookExecutor::eval_condition("on_branch:main", &context).unwrap());
        assert!(HookExecutor::eval_condition("on_branch:*", &context).unwrap());
    }

    #[test]
    fn test_check_tool_available() {
        let has_echo = HookExecutor::check_tool_available("echo").unwrap();
        assert!(has_echo); // echo should always be available
    }

    #[test]
    fn test_check_tool_unavailable() {
        let has_nonexistent =
            HookExecutor::check_tool_available("totally_nonexistent_tool_xyz").unwrap_or(false);
        assert!(!has_nonexistent);
    }
}
