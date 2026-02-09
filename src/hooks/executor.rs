use super::{Action, Hook};
use crate::events::EventContext;
use std::process::Command;

pub struct HookExecutor;

/// Approval callback type - returns true if user approves, false if declined
pub type ApprovalCallback = Box<dyn Fn(&str) -> bool + Send + Sync>;

#[derive(Debug, Clone)]
pub struct HookResult {
    pub hook_name: String,
    pub action_index: usize,
    pub output: String,
    pub inject_key: Option<String>,
}

impl HookExecutor {
    /// Execute a single hook and collect results (no approval gates)
    pub fn execute_hook(hook: &Hook, context: &EventContext) -> anyhow::Result<Vec<HookResult>> {
        Self::execute_hook_with_approval(hook, context, None)
    }

    /// Execute a hook with optional approval callback
    pub fn execute_hook_with_approval(
        hook: &Hook,
        context: &EventContext,
        approval_fn: Option<&ApprovalCallback>,
    ) -> anyhow::Result<Vec<HookResult>> {
        let mut results = Vec::new();

        // Check condition if present
        if let Some(condition) = &hook.condition {
            if !Self::eval_condition(condition, context)? {
                return Ok(results); // Skip hook if condition fails
            }
        }

        // Execute each action
        for (idx, action) in hook.actions.iter().enumerate() {
            if let Some(result) = Self::execute_action(action, context, approval_fn)? {
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
        approval_fn: Option<&ApprovalCallback>,
    ) -> anyhow::Result<Option<(String, Option<String>)>> {
        match action {
            Action::Command {
                command,
                inject_as,
                requires_approval,
                approval_prompt,
            } => {
                // Check if approval is required
                if *requires_approval {
                    let prompt = approval_prompt
                        .as_ref()
                        .map(|s| s.as_str())
                        .unwrap_or_else(|| command.as_str());

                    if let Some(callback) = approval_fn {
                        if !callback(prompt) {
                            // User declined - skip this action
                            return Ok(None);
                        }
                    } else {
                        // No approval callback provided but approval required
                        // For safety, skip the action
                        crate::ui::warn(format!(
                            "Warning: Action requires approval but no callback provided. Skipping: {command}"
                        ));
                        return Ok(None);
                    }
                }

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
                        Self::execute_action(action, context, approval_fn)?;
                    }
                }
                Ok(None)
            }
            Action::Confirm { .. }
            | Action::Prompt { .. }
            | Action::SecretPrompt { .. }
            | Action::SetEnv { .. }
            | Action::SetConfig { .. } => Ok(None),
        }
    }

    /// Run a shell command and capture output
    fn run_command(command_str: &str) -> anyhow::Result<String> {
        let output = Command::new("sh").arg("-c").arg(command_str).output()?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            crate::ui::warn(format!("Hook command failed: {stderr}"));
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
            return Self::check_tool_available(tool);
        }

        // Unknown condition -> fail closed.
        crate::ui::warn(format!(
            "Warning: Unknown hook condition '{condition}'; skipping hook for safety"
        ));
        Ok(false)
    }

    /// Check if a tool is available in PATH
    fn check_tool_available(tool: &str) -> anyhow::Result<bool> {
        Ok(crate::plugins::system().has_in_path(tool))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn create_test_hook_yaml(content: &str) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

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
    fn test_condition_unknown_fails_closed() {
        let context = EventContext::new();
        assert!(!HookExecutor::eval_condition("unknown_condition:foo", &context).unwrap());
    }

    #[test]
    fn test_check_tool_available() {
        let has_echo = HookExecutor::check_tool_available("echo").unwrap();
        assert!(has_echo);
    }

    #[test]
    fn test_check_tool_unavailable() {
        let has_nonexistent =
            HookExecutor::check_tool_available("totally_nonexistent_tool_xyz").unwrap_or(false);
        assert!(!has_nonexistent);
    }

    #[test]
    fn test_hook_with_unknown_condition_is_skipped() {
        let yaml = r#"name: test_unknown_condition
trigger: SessionStart
condition: unknown_condition:foo
actions:
  - type: command
    command: echo should_not_run
"#;
        let file = create_test_hook_yaml(yaml);
        let hook = crate::hooks::parse_hook(file.path()).unwrap();
        let context = EventContext::new();

        let results = HookExecutor::execute_hook(&hook, &context).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_conditional_action_with_unknown_condition_is_skipped() {
        let yaml = r#"name: test_unknown_action_condition
trigger: SessionStart
actions:
  - type: conditional
    condition: unknown_condition:foo
    then:
      - type: command
        command: echo should_not_run
"#;
        let file = create_test_hook_yaml(yaml);
        let hook = crate::hooks::parse_hook(file.path()).unwrap();
        let context = EventContext::new();

        let results = HookExecutor::execute_hook(&hook, &context).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_approval_required_approved() {
        let yaml = r#"name: test_approval
trigger: SessionStart
actions:
  - type: command
    command: echo approved
    requires_approval: true
    approval_prompt: Run this command?
"#;
        let file = create_test_hook_yaml(yaml);
        let hook = crate::hooks::parse_hook(file.path()).unwrap();
        let context = EventContext::new();

        let approve_fn: ApprovalCallback = Box::new(|_| true);
        let results =
            HookExecutor::execute_hook_with_approval(&hook, &context, Some(&approve_fn)).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].output, "approved");
    }

    #[test]
    fn test_approval_required_declined() {
        let yaml = r#"name: test_decline
trigger: SessionStart
actions:
  - type: command
    command: echo should_not_run
    requires_approval: true
"#;
        let file = create_test_hook_yaml(yaml);
        let hook = crate::hooks::parse_hook(file.path()).unwrap();
        let context = EventContext::new();

        let decline_fn: ApprovalCallback = Box::new(|_| false);
        let results =
            HookExecutor::execute_hook_with_approval(&hook, &context, Some(&decline_fn)).unwrap();

        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_approval_required_no_callback() {
        let yaml = r#"name: test_no_callback
trigger: SessionStart
actions:
  - type: command
    command: echo should_not_run
    requires_approval: true
"#;
        let file = create_test_hook_yaml(yaml);
        let hook = crate::hooks::parse_hook(file.path()).unwrap();
        let context = EventContext::new();

        let results = HookExecutor::execute_hook_with_approval(&hook, &context, None).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_no_approval_required() {
        let yaml = r#"name: test_no_approval
trigger: SessionStart
actions:
  - type: command
    command: echo no_approval_needed
"#;
        let file = create_test_hook_yaml(yaml);
        let hook = crate::hooks::parse_hook(file.path()).unwrap();
        let context = EventContext::new();

        let results = HookExecutor::execute_hook(&hook, &context).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].output, "no_approval_needed");
    }

    #[test]
    fn test_approval_custom_prompt() {
        let yaml = r#"name: test_custom
trigger: SessionStart
actions:
  - type: command
    command: echo test
    requires_approval: true
    approval_prompt: Custom approval message
"#;
        let file = create_test_hook_yaml(yaml);
        let hook = crate::hooks::parse_hook(file.path()).unwrap();
        let context = EventContext::new();

        let captured = Arc::new(Mutex::new(String::new()));
        let captured_clone = Arc::clone(&captured);

        let callback: ApprovalCallback = Box::new(move |prompt| {
            *captured_clone.lock().unwrap() = prompt.to_string();
            true
        });

        HookExecutor::execute_hook_with_approval(&hook, &context, Some(&callback)).unwrap();
        assert_eq!(*captured.lock().unwrap(), "Custom approval message");
    }

    #[test]
    fn test_multiple_actions_mixed_approval() {
        let yaml = r#"name: test_mixed
trigger: SessionStart
actions:
  - type: command
    command: echo always_runs
  - type: command
    command: echo needs_approval
    requires_approval: true
  - type: command
    command: echo also_always_runs
"#;
        let file = create_test_hook_yaml(yaml);
        let hook = crate::hooks::parse_hook(file.path()).unwrap();
        let context = EventContext::new();

        // Decline the approval-required action
        let decline_fn: ApprovalCallback = Box::new(|_| false);
        let results =
            HookExecutor::execute_hook_with_approval(&hook, &context, Some(&decline_fn)).unwrap();

        // Should have 2 results (skipped the approval-required one)
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].output, "always_runs");
        assert_eq!(results[1].output, "also_always_runs");
    }
}
