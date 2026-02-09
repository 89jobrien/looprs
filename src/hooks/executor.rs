use super::{Action, Hook, PromptCallback};
use crate::app_config::AppConfig;
use crate::events::EventContext;
use crate::state::AppState;
use std::collections::HashMap;
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
        Self::execute_hook_with_callbacks(hook, context, None, None, None)
    }

    /// Execute a hook with optional approval callback
    pub fn execute_hook_with_approval(
        hook: &Hook,
        context: &EventContext,
        approval_fn: Option<&ApprovalCallback>,
    ) -> anyhow::Result<Vec<HookResult>> {
        Self::execute_hook_with_callbacks(hook, context, approval_fn, None, None)
    }

    /// Execute a hook with approval and prompt callbacks
    pub fn execute_hook_with_callbacks(
        hook: &Hook,
        context: &EventContext,
        approval_fn: Option<&ApprovalCallback>,
        prompt_fn: Option<&PromptCallback>,
        secret_prompt_fn: Option<&PromptCallback>,
    ) -> anyhow::Result<Vec<HookResult>> {
        let mut results = Vec::new();
        let mut local_ctx: HashMap<String, String> = HashMap::new();

        // Check condition if present
        if let Some(condition) = &hook.condition {
            if !Self::eval_condition(condition, &local_ctx)? {
                return Ok(results); // Skip hook if condition fails
            }
        }

        // Execute each action
        for (idx, action) in hook.actions.iter().enumerate() {
            if let Some(result) = Self::execute_action(
                action,
                context,
                approval_fn,
                prompt_fn,
                secret_prompt_fn,
                &mut local_ctx,
            )? {
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
        _context: &EventContext,
        approval_fn: Option<&ApprovalCallback>,
        prompt_fn: Option<&PromptCallback>,
        secret_prompt_fn: Option<&PromptCallback>,
        local_ctx: &mut HashMap<String, String>,
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
            Action::Message { text } => {
                crate::ui::info(text);
                Ok(Some((text.clone(), None)))
            }
            Action::Conditional {
                condition,
                then: actions,
            } => {
                if Self::eval_condition(condition, local_ctx)? {
                    let mut last_result: Option<(String, Option<String>)> = None;
                    for action in actions {
                        if let Some(result) = Self::execute_action(
                            action,
                            _context,
                            approval_fn,
                            prompt_fn,
                            secret_prompt_fn,
                            local_ctx,
                        )? {
                            last_result = Some(result);
                        }
                    }
                    return Ok(last_result);
                }
                Ok(None)
            }
            Action::Confirm { prompt, set_key } => {
                let approved = if let Some(callback) = approval_fn {
                    callback(prompt)
                } else {
                    crate::ui::warn(
                        "Warning: Confirm action requires approval callback; defaulting to false",
                    );
                    false
                };
                local_ctx.insert(set_key.clone(), approved.to_string());
                Ok(None)
            }
            Action::Prompt { prompt, set_key } => {
                if let Some(callback) = prompt_fn {
                    if let Some(value) = callback(prompt) {
                        local_ctx.insert(set_key.clone(), value);
                    }
                } else {
                    crate::ui::warn("Warning: Prompt action requires a prompt callback; skipping");
                }
                Ok(None)
            }
            Action::SecretPrompt { prompt, set_key } => {
                if let Some(callback) = secret_prompt_fn {
                    if let Some(value) = callback(prompt) {
                        local_ctx.insert(set_key.clone(), value);
                    }
                } else {
                    crate::ui::warn(
                        "Warning: Secret prompt action requires a prompt callback; skipping",
                    );
                }
                Ok(None)
            }
            Action::SetEnv { name, from_key } => {
                if let Some(value) = local_ctx.get(from_key) {
                    if !value.is_empty() {
                        unsafe {
                            std::env::set_var(name, value);
                        }
                    }
                } else {
                    crate::ui::warn(format!(
                        "Warning: set_env missing key '{from_key}'; skipping"
                    ));
                }
                Ok(None)
            }
            Action::SetConfig { path, value } => {
                if path == "onboarding.demo_seen" {
                    let Some(flag) = value.as_bool() else {
                        crate::ui::warn(format!("Warning: set_config expected boolean for {path}"));
                        return Ok(None);
                    };
                    AppState::set_onboarding_demo_seen(flag)?;
                } else {
                    crate::ui::warn(format!("Warning: Unknown config path '{path}'"));
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
            crate::ui::warn(format!("Hook command failed: {stderr}"));
        }

        Ok(stdout.trim().to_string())
    }

    /// Evaluate simple conditions (very basic for now)
    fn eval_condition(
        condition: &str,
        local_ctx: &HashMap<String, String>,
    ) -> anyhow::Result<bool> {
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

        if let Some(rest) = condition.strip_prefix("equals:") {
            let parts: Vec<&str> = rest.splitn(2, ':').collect();
            if parts.len() == 2 {
                return Ok(local_ctx
                    .get(parts[0])
                    .map(|v| v == parts[1])
                    .unwrap_or(false));
            }
        }

        if let Some(var) = condition.strip_prefix("env_set:") {
            return Ok(std::env::var(var).map(|v| !v.is_empty()).unwrap_or(false));
        }

        if let Some(rest) = condition.strip_prefix("config_flag:") {
            let parts: Vec<&str> = rest.splitn(2, '=').collect();
            if parts.len() == 2 {
                let cfg = match AppConfig::load() {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        crate::ui::warn(format!(
                            "Warning: Failed to load config for condition '{condition}': {e}"
                        ));
                        return Ok(false);
                    }
                };
                if parts[0] == "onboarding.demo_seen" {
                    return Ok(cfg.onboarding.demo_seen.to_string() == parts[1]);
                }
            }
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
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, OnceLock};
    use tempfile::TempDir;

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("lock test mutex")
    }

    struct DirGuard {
        original: PathBuf,
    }

    impl DirGuard {
        fn change_to(path: &std::path::Path) -> Self {
            let original = std::env::current_dir().expect("read current dir");
            std::env::set_current_dir(path).expect("set current dir");
            Self { original }
        }
    }

    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

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
        let local_ctx: HashMap<String, String> = HashMap::new();
        assert!(HookExecutor::eval_condition("on_branch:main", &local_ctx).unwrap());
        assert!(HookExecutor::eval_condition("on_branch:*", &local_ctx).unwrap());
    }

    #[test]
    fn test_condition_unknown_fails_closed() {
        let local_ctx: HashMap<String, String> = HashMap::new();
        assert!(!HookExecutor::eval_condition("unknown_condition:foo", &local_ctx).unwrap());
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
    fn test_condition_env_set() {
        let _lock = test_lock();
        let local_ctx: HashMap<String, String> = HashMap::new();
        let key = "LOOPRS_TEST_ENV_SET";
        let prev = std::env::var(key).ok();

        unsafe {
            std::env::set_var(key, "1");
        }
        assert!(HookExecutor::eval_condition("env_set:LOOPRS_TEST_ENV_SET", &local_ctx).unwrap());

        unsafe {
            std::env::set_var(key, "");
        }
        assert!(!HookExecutor::eval_condition("env_set:LOOPRS_TEST_ENV_SET", &local_ctx).unwrap());

        unsafe {
            std::env::remove_var(key);
        }
        assert!(!HookExecutor::eval_condition("env_set:LOOPRS_TEST_ENV_SET", &local_ctx).unwrap());

        unsafe {
            if let Some(value) = prev {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }

    #[test]
    fn test_condition_config_flag() {
        let _lock = test_lock();
        let tmp = TempDir::new().unwrap();
        let _guard = DirGuard::change_to(tmp.path());
        let local_ctx: HashMap<String, String> = HashMap::new();

        std::fs::create_dir_all(".looprs").unwrap();
        std::fs::write(
            ".looprs/state.json",
            r#"{ "onboarding": { "demo_seen": true } }"#,
        )
        .unwrap();

        assert!(
            HookExecutor::eval_condition("config_flag:onboarding.demo_seen=true", &local_ctx)
                .unwrap()
        );
        assert!(
            !HookExecutor::eval_condition("config_flag:onboarding.demo_seen=false", &local_ctx)
                .unwrap()
        );
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
    fn equals_condition_uses_hook_local_context() {
        let yaml = r#"name: test
trigger: SessionStart
actions:
  - type: confirm
    prompt: "Continue?"
    set_key: continue
  - type: conditional
    condition: equals:continue:true
    then:
      - type: message
        text: "ok"
"#;
        let file = create_test_hook_yaml(yaml);
        let hook = crate::hooks::parse_hook(file.path()).unwrap();
        let context = EventContext::new();

        let approve: ApprovalCallback = Box::new(|_| true);
        let results =
            HookExecutor::execute_hook_with_approval(&hook, &context, Some(&approve)).unwrap();
        assert!(results.iter().any(|r| r.output == "ok"));
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

    #[test]
    fn secret_prompt_does_not_inject_metadata() {
        let yaml = r#"name: test
trigger: SessionStart
actions:
  - type: secret_prompt
    prompt: "Key"
    set_key: key
  - type: set_env
    name: OPENAI_API_KEY
    from_key: key
"#;
        let file = create_test_hook_yaml(yaml);
        let hook = crate::hooks::parse_hook(file.path()).unwrap();
        let context = EventContext::new();

        let secret: crate::hooks::PromptCallback = Box::new(|_| Some("secret".to_string()));
        let results =
            HookExecutor::execute_hook_with_callbacks(&hook, &context, None, None, Some(&secret))
                .unwrap();

        assert!(results.iter().all(|r| r.inject_key.is_none()));
    }

    #[test]
    fn set_env_sets_value_from_prompt() {
        let _lock = test_lock();
        let key = "LOOPRS_TEST_SET_ENV";
        let prev = std::env::var(key).ok();

        let yaml = r#"name: test
trigger: SessionStart
actions:
  - type: prompt
    prompt: "Value"
    set_key: value
  - type: set_env
    name: LOOPRS_TEST_SET_ENV
    from_key: value
"#;
        let file = create_test_hook_yaml(yaml);
        let hook = crate::hooks::parse_hook(file.path()).unwrap();
        let context = EventContext::new();
        let prompt: crate::hooks::PromptCallback = Box::new(|_| Some("ok".to_string()));

        HookExecutor::execute_hook_with_callbacks(&hook, &context, None, Some(&prompt), None)
            .unwrap();

        assert_eq!(std::env::var(key).unwrap(), "ok");

        unsafe {
            if let Some(value) = prev {
                std::env::set_var(key, value);
            } else {
                std::env::remove_var(key);
            }
        }
    }

    #[test]
    fn set_config_sets_onboarding_flag() {
        let _lock = test_lock();
        let tmp = TempDir::new().unwrap();
        let _guard = DirGuard::change_to(tmp.path());

        let yaml = r#"name: test
trigger: SessionStart
actions:
  - type: set_config
    path: onboarding.demo_seen
    value: true
"#;
        let file = create_test_hook_yaml(yaml);
        let hook = crate::hooks::parse_hook(file.path()).unwrap();
        let context = EventContext::new();

        HookExecutor::execute_hook_with_callbacks(&hook, &context, None, None, None).unwrap();

        let saved = std::fs::read_to_string(".looprs/state.json").unwrap();
        assert!(saved.contains("\"demo_seen\": true"));
    }
}
