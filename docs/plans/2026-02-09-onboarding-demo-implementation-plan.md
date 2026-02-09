# Demo Onboarding Wizard Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a hook-driven onboarding wizard that runs once by default, sets provider API keys for the session only, and allows disabling via `.looprs/config.json`.

**Architecture:** Extend hook actions/conditions to support interactive prompts and config/env updates. Add a repo hook that drives the wizard flow on `SessionStart`, guarded by a config flag.

**Tech Stack:** Rust, serde_yaml/serde_json, existing hook executor and REPL callbacks.

---

### Task 1: Add onboarding flag support in config

**Files:**
- Modify: `src/app_config.rs`
- Test: `src/app_config.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn onboarding_demo_seen_defaults_false() {
    let cfg = AppConfig::default();
    assert!(!cfg.onboarding.demo_seen);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib app_config::tests::onboarding_demo_seen_defaults_false`
Expected: FAIL with “no field onboarding on type AppConfig”

**Step 3: Write minimal implementation**

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppConfig {
    pub defaults: DefaultsConfig,
    pub file_references: FileReferencesConfig,
    pub onboarding: OnboardingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OnboardingConfig {
    pub demo_seen: bool,
}

impl Default for OnboardingConfig {
    fn default() -> Self {
        Self { demo_seen: false }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --lib app_config::tests::onboarding_demo_seen_defaults_false`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app_config.rs
git commit -m "feat(config): add onboarding demo flag"
```

---

### Task 2: Add config update helper that preserves unknown keys

**Files:**
- Modify: `src/app_config.rs`
- Test: `src/app_config.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn set_onboarding_demo_seen_preserves_unknown_fields() {
    use std::fs;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    std::env::set_current_dir(tmp.path()).unwrap();
    fs::create_dir_all(".looprs").unwrap();
    fs::write(
        ".looprs/config.json",
        r#"{ "version": "1.0.0", "onboarding": {"demo_seen": false} }"#,
    )
    .unwrap();

    AppConfig::set_onboarding_demo_seen(true).unwrap();

    let saved = fs::read_to_string(".looprs/config.json").unwrap();
    assert!(saved.contains("\"version\": \"1.0.0\""));
    assert!(saved.contains("\"demo_seen\": true"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib app_config::tests::set_onboarding_demo_seen_preserves_unknown_fields`
Expected: FAIL with “no function or associated item named set_onboarding_demo_seen”

**Step 3: Write minimal implementation**

```rust
impl AppConfig {
    pub fn set_onboarding_demo_seen(value: bool) -> anyhow::Result<()> {
        use serde_json::{json, Value};
        let path = Path::new(".looprs/config.json");
        let mut root: Value = if path.exists() {
            serde_json::from_str(&fs::read_to_string(path)?)?
        } else {
            json!({})
        };
        if !root.is_object() {
            root = json!({});
        }
        let obj = root.as_object_mut().unwrap();
        let onboarding = obj
            .entry("onboarding")
            .or_insert_with(|| json!({}));
        onboarding
            .as_object_mut()
            .unwrap()
            .insert("demo_seen".to_string(), json!(value));

        fs::create_dir_all(".looprs")?;
        fs::write(path, serde_json::to_string_pretty(&root)?)?;
        Ok(())
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --lib app_config::tests::set_onboarding_demo_seen_preserves_unknown_fields`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app_config.rs
git commit -m "feat(config): update onboarding flag safely"
```

---

### Task 3: Add interactive prompt callbacks

**Files:**
- Modify: `src/approval.rs`
- Modify: `src/lib.rs`
- Test: `src/approval.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn prompt_functions_exist() {
    let _p = console_prompt;
    let _s = console_secret_prompt;
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib approval::tests::prompt_functions_exist`
Expected: FAIL with “cannot find value `console_prompt` in this scope”

**Step 3: Write minimal implementation**

```rust
pub fn console_prompt(message: &str) -> Option<String> {
    print!("{message} ");
    if io::stdout().flush().is_err() {
        return None;
    }
    let mut input = String::new();
    if io::stdin().read_line(&mut input).is_err() {
        return None;
    }
    let trimmed = input.trim().to_string();
    if trimmed.is_empty() { None } else { Some(trimmed) }
}

pub fn console_secret_prompt(message: &str) -> Option<String> {
    print!("{message} ");
    if io::stdout().flush().is_err() {
        return None;
    }
    match rpassword::read_password() {
        Ok(s) => {
            let trimmed = s.trim().to_string();
            if trimmed.is_empty() { None } else { Some(trimmed) }
        }
        Err(_) => None,
    }
}
```

Also add `rpassword = "7"` to `Cargo.toml`.

**Step 4: Run test to verify it passes**

Run: `cargo test --lib approval::tests::prompt_functions_exist`
Expected: PASS

**Step 5: Commit**

```bash
git add src/approval.rs Cargo.toml src/lib.rs
git commit -m "feat(approval): add console prompts"
```

---

### Task 4: Extend hook action schema

**Files:**
- Modify: `src/hooks/mod.rs`
- Test: `src/hooks/parser.rs`

**Step 1: Write the failing test**

```rust
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
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib hooks::parser::tests::parse_new_hook_actions`
Expected: FAIL with “unknown variant `confirm`”

**Step 3: Write minimal implementation**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Action {
    #[serde(rename = "command")]
    Command { /* existing fields */ },
    #[serde(rename = "message")]
    Message { text: String },
    #[serde(rename = "conditional")]
    Conditional { condition: String, #[serde(default)] then: Vec<Action> },
    #[serde(rename = "confirm")]
    Confirm { prompt: String, set_key: String },
    #[serde(rename = "prompt")]
    Prompt { prompt: String, set_key: String },
    #[serde(rename = "secret_prompt")]
    SecretPrompt { prompt: String, set_key: String },
    #[serde(rename = "set_env")]
    SetEnv { name: String, from_key: String },
    #[serde(rename = "set_config")]
    SetConfig { path: String, value: serde_json::Value },
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --lib hooks::parser::tests::parse_new_hook_actions`
Expected: PASS

**Step 5: Commit**

```bash
git add src/hooks/mod.rs src/hooks/parser.rs
git commit -m "feat(hooks): add onboarding action types"
```

---

### Task 5: Add hook-local context and new condition evaluators

**Files:**
- Modify: `src/hooks/executor.rs`
- Test: `src/hooks/executor.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn equals_condition_uses_hook_local_context() {
    let yaml = r#"name: test
trigger: SessionStart
actions:
  - type: confirm
    prompt: \"Continue?\"
    set_key: continue
  - type: conditional
    condition: equals:continue:true
    then:
      - type: message
        text: \"ok\"
"#;
    let file = create_test_hook_yaml(yaml);
    let hook = crate::hooks::parse_hook(file.path()).unwrap();
    let context = EventContext::new();

    let approve: ApprovalCallback = Box::new(|_| true);
    let results = HookExecutor::execute_hook_with_approval(&hook, &context, Some(&approve)).unwrap();
    assert!(results.iter().any(|r| r.output == "ok"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib hooks::executor::tests::equals_condition_uses_hook_local_context`
Expected: FAIL with “unknown hook condition” or no results

**Step 3: Write minimal implementation**

- Add `local_ctx: HashMap<String, String>` inside `execute_hook_with_approval`.
- Update `eval_condition` to accept `local_ctx` and parse:
  - `equals:<key>:<value>`
  - `env_set:VAR`
  - `config_flag:path=value` (use `AppConfig::load()`)
- Update `Conditional` action execution to pass `local_ctx`.

```rust
fn eval_condition(
    condition: &str,
    local_ctx: &HashMap<String, String>,
) -> anyhow::Result<bool> {
    if let Some(rest) = condition.strip_prefix("equals:") {
        let parts: Vec<&str> = rest.splitn(2, ':').collect();
        if parts.len() == 2 {
            return Ok(local_ctx.get(parts[0]).map(|v| v == parts[1]).unwrap_or(false));
        }
    }
    if let Some(var) = condition.strip_prefix("env_set:") {
        return Ok(std::env::var(var).map(|v| !v.is_empty()).unwrap_or(false));
    }
    if let Some(rest) = condition.strip_prefix("config_flag:") {
        let parts: Vec<&str> = rest.splitn(2, '=').collect();
        if parts.len() == 2 {
            let cfg = AppConfig::load().unwrap_or_default();
            if parts[0] == "onboarding.demo_seen" {
                return Ok(cfg.onboarding.demo_seen.to_string() == parts[1]);
            }
        }
    }
    // existing conditions…
    Ok(false)
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --lib hooks::executor::tests::equals_condition_uses_hook_local_context`
Expected: PASS

**Step 5: Commit**

```bash
git add src/hooks/executor.rs
 git commit -m "feat(hooks): add local context conditions"
```

---

### Task 6: Implement new hook actions execution

**Files:**
- Modify: `src/hooks/executor.rs`
- Test: `src/hooks/executor.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn secret_prompt_does_not_inject_metadata() {
    let yaml = r#"name: test
trigger: SessionStart
actions:
  - type: secret_prompt
    prompt: \"Key\"
    set_key: key
  - type: set_env
    name: OPENAI_API_KEY
    from_key: key
"#;
    let file = create_test_hook_yaml(yaml);
    let hook = crate::hooks::parse_hook(file.path()).unwrap();
    let context = EventContext::new();

    let secret: PromptCallback = Box::new(|_| Some("secret".to_string()));
    let results = HookExecutor::execute_hook_with_callbacks(&hook, &context, None, None, Some(&secret)).unwrap();

    assert!(results.iter().all(|r| r.inject_key.is_none()));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib hooks::executor::tests::secret_prompt_does_not_inject_metadata`
Expected: FAIL with “no method execute_hook_with_callbacks”

**Step 3: Write minimal implementation**

- Add new callback types in `src/hooks/mod.rs`:

```rust
pub type PromptCallback = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;
```

- Add a new executor entrypoint:

```rust
pub fn execute_hook_with_callbacks(
    hook: &Hook,
    context: &EventContext,
    approval_fn: Option<&ApprovalCallback>,
    prompt_fn: Option<&PromptCallback>,
    secret_prompt_fn: Option<&PromptCallback>,
) -> anyhow::Result<Vec<HookResult>> { /* ... */ }
```

- Implement actions:
  - `Confirm`: use approval callback, store `true/false` in local_ctx (string).
  - `Prompt` / `SecretPrompt`: use prompt callback; store in local_ctx only.
  - `SetEnv`: read from local_ctx, `std::env::set_var` if non-empty.
  - `SetConfig`: call `AppConfig::set_onboarding_demo_seen` when path matches.
  - `Message`: call `ui::info` so messages are visible.

**Step 4: Run test to verify it passes**

Run: `cargo test --lib hooks::executor::tests::secret_prompt_does_not_inject_metadata`
Expected: PASS

**Step 5: Commit**

```bash
git add src/hooks/executor.rs src/hooks/mod.rs
git commit -m "feat(hooks): execute onboarding actions"
```

---

### Task 7: Wire prompt callbacks into SessionStart execution

**Files:**
- Modify: `src/bin/looprs/main.rs`
- Modify: `src/agent.rs`
- Modify: `src/lib.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn execute_hooks_supports_prompt_callbacks() {
    let _fn = Agent::execute_hooks_for_event_with_callbacks;
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib agent::tests::execute_hooks_supports_prompt_callbacks`
Expected: FAIL with “no function or associated item named execute_hooks_for_event_with_callbacks”

**Step 3: Write minimal implementation**

- Add new method on Agent:

```rust
pub fn execute_hooks_for_event_with_callbacks(
    &self,
    event: &Event,
    context: &EventContext,
    approval_fn: Option<&ApprovalCallback>,
    prompt_fn: Option<&PromptCallback>,
    secret_prompt_fn: Option<&PromptCallback>,
) -> EventContext { /* delegate to HookExecutor */ }
```

- In `run_interactive`, construct callbacks:

```rust
let approval_callback: ApprovalCallback = Box::new(console_approval_prompt);
let prompt_callback: PromptCallback = Box::new(console_prompt);
let secret_prompt_callback: PromptCallback = Box::new(console_secret_prompt);

let enriched_ctx = agent.execute_hooks_for_event_with_callbacks(
    &Event::SessionStart,
    &event_ctx,
    Some(&approval_callback),
    Some(&prompt_callback),
    Some(&secret_prompt_callback),
);
```

**Step 4: Run test to verify it passes**

Run: `cargo test --lib agent::tests::execute_hooks_supports_prompt_callbacks`
Expected: PASS

**Step 5: Commit**

```bash
git add src/bin/looprs/main.rs src/agent.rs src/lib.rs
git commit -m "feat(hooks): pass prompt callbacks"
```

---

### Task 8: Add demo onboarding hook and update docs

**Files:**
- Create: `.looprs/hooks/demo_onboarding.yaml`
- Modify: `.looprs/hooks/README.md`
- Modify: `.looprs/README.md`

**Step 1: Write the failing test**

```rust
#[test]
fn demo_onboarding_hook_parses() {
    let hook = crate::hooks::parse_hook(Path::new(".looprs/hooks/demo_onboarding.yaml")).unwrap();
    assert_eq!(hook.name, "demo_onboarding");
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib hooks::parser::tests::demo_onboarding_hook_parses`
Expected: FAIL with “No such file or directory”

**Step 3: Write minimal implementation**

Create `.looprs/hooks/demo_onboarding.yaml` with the wizard flow (session-only keys, persistent guidance, disable flag). Update `.looprs/hooks/README.md` and `.looprs/README.md` to document the new action types and demo hook.

**Step 4: Run test to verify it passes**

Run: `cargo test --lib hooks::parser::tests::demo_onboarding_hook_parses`
Expected: PASS

**Step 5: Commit**

```bash
git add .looprs/hooks/demo_onboarding.yaml .looprs/hooks/README.md .looprs/README.md
 git commit -m "docs: add demo onboarding hook"
```

---

### Task 9: Full test pass

**Files:**
- None

**Step 1: Run tests**

Run: `cargo test --lib`
Expected: PASS

**Step 2: Run clippy (optional)**

Run: `cargo clippy --all-targets -- -D warnings`
Expected: PASS (or note pre-existing failures)

**Step 3: Commit (if needed)**

```bash
# Only if changes from fixing failures
git add -A
git commit -m "test: fix clippy/test issues"
```

---

Plan complete and saved to `docs/plans/2026-02-09-onboarding-demo-implementation-plan.md`.

Two execution options:

1. Subagent-Driven (this session) — I dispatch a fresh subagent per task, review between tasks, fast iteration
2. Parallel Session (separate) — Open new session with executing-plans, batch execution with checkpoints

Which approach?
