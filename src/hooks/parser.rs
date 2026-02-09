use super::Hook;
use std::fs;
use std::path::Path;

/// Parse a YAML hook file
pub fn parse_hook(path: &Path) -> anyhow::Result<Hook> {
    let contents = fs::read_to_string(path)?;
    let hook: Hook = serde_yaml::from_str(&contents)?;
    Ok(hook)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_simple_hook() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"name: test_hook
trigger: SessionStart
actions:
  - type: message
    text: "Hello from hook""#
        )
        .unwrap();

        let hook = parse_hook(file.path()).unwrap();
        assert_eq!(hook.name, "test_hook");
        assert_eq!(hook.trigger, "SessionStart");
        assert_eq!(hook.actions.len(), 1);
    }

    #[test]
    fn test_parse_command_hook() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"name: git_status
trigger: PostToolUse
actions:
  - type: command
    command: "git status"
    inject_as: "git_info""#
        )
        .unwrap();

        let hook = parse_hook(file.path()).unwrap();
        assert_eq!(hook.name, "git_status");
        assert_eq!(hook.trigger, "PostToolUse");
        assert_eq!(hook.actions.len(), 1);
    }

    #[test]
    fn parse_new_hook_actions() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"name: demo
trigger: SessionStart
actions:
  - type: confirm
    prompt: \"Continue?\"
    set_key: continue
  - type: secret_prompt
    prompt: \"Key\"
    set_key: key
  - type: set_env
    name: OPENAI_API_KEY
    from_key: key
  - type: set_config
    path: onboarding.demo_seen
    value: true
"#
        )
        .unwrap();

        let hook = parse_hook(file.path()).unwrap();
        assert_eq!(hook.actions.len(), 4);
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"name: broken
trigger: [invalid yaml"#
        )
        .unwrap();

        let result = parse_hook(file.path());
        assert!(result.is_err());
    }

    #[test]
    fn parse_new_hook_actions() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"name: demo
trigger: SessionStart
actions:
  - type: confirm
    prompt: "Continue?"
    set_key: continue
  - type: secret_prompt
    prompt: "Key"
    set_key: key
  - type: set_env
    name: OPENAI_API_KEY
    from_key: key
  - type: set_config
    path: onboarding.demo_seen
    value: true
"#
        )
        .unwrap();

        let hook = parse_hook(file.path()).unwrap();
        assert_eq!(hook.actions.len(), 4);
    }
}
