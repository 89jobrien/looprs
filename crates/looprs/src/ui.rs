use colored::*;

use crate::automation_protocol;
use crate::observability;
use crate::sanitize;

pub fn init_logging() {
    // C2a: internal logs are opt-in via RUST_LOG. UI output remains separate.
    let mut builder = env_logger::Builder::from_default_env();
    // If user hasn't set RUST_LOG, default to warnings+.
    if std::env::var("RUST_LOG").is_err() {
        builder.filter_level(log::LevelFilter::Warn);
    }
    let _ = builder.try_init();

    let root = observability::observability_root();
    if std::fs::create_dir_all(&root).is_ok() {
        let _ = observability::append_named_jsonl(
            "runtime",
            &serde_json::json!({
                "kind": "logging_init",
                "observability_root": root.display().to_string(),
            }),
        );
    }
}

fn emit_machine_event(kind: &str, data: serde_json::Value) {
    let mut stderr = std::io::stderr().lock();
    let service = automation_protocol::AutomationProtocol::system();
    if let Ok(Some(event)) = write_machine_event(&service, &mut stderr, kind, data) {
        let _ = observability::append_named_jsonl("ui_events", &event);
    }
}

fn write_machine_event(
    service: &automation_protocol::AutomationProtocol<'_>,
    writer: &mut impl std::io::Write,
    kind: &str,
    data: serde_json::Value,
) -> std::io::Result<Option<serde_json::Value>> {
    let mut sink = automation_protocol::JsonLineEventSink::new(writer);
    let Some(record) = service.emit(&mut sink, kind, data)? else {
        return Ok(None);
    };
    let event = serde_json::to_value(record)?;
    Ok(Some(event))
}

/// Emits one machine event as JSONL on stderr when machine output is enabled.
///
/// `--machine-log` uses the legacy top-level `{kind,data}` shape. An explicit
/// `--machine-protocol looprs-machine/v1` emits a versioned envelope. Human and
/// assistant output remains on stdout.
///
/// # Examples
///
/// ```no_run
/// use looprs::automation_protocol::{MACHINE_PROTOCOL_ENV, MACHINE_PROTOCOL_V1};
///
/// // SAFETY: configure process-global output before starting worker threads.
/// unsafe { std::env::set_var(MACHINE_PROTOCOL_ENV, MACHINE_PROTOCOL_V1) };
/// looprs::ui::machine_event("run.started", serde_json::json!({"scriptable": true}));
/// ```
pub fn machine_event(kind: &str, data: serde_json::Value) {
    emit_machine_event(kind, data);
}

pub fn info(msg: impl AsRef<str>) {
    let raw = msg.as_ref();
    println!("{}", sanitize::sanitize_preview_for_console(raw));
    emit_machine_event("info", serde_json::json!({ "message": raw }));
}

pub fn info_full(msg: impl AsRef<str>) {
    println!("{}", sanitize::sanitize_for_console(msg.as_ref()));
}

pub fn warn(msg: impl AsRef<str>) {
    let raw = msg.as_ref();
    eprintln!("{}", sanitize::sanitize_preview_for_console(raw));
    emit_machine_event("warn", serde_json::json!({ "message": raw }));
}

pub fn error(msg: impl AsRef<str>) {
    let raw = msg.as_ref();
    eprintln!("{}", sanitize::sanitize_preview_for_console(raw));
    emit_machine_event("error", serde_json::json!({ "message": raw }));
}

pub fn error_full(msg: impl AsRef<str>) {
    eprintln!("{}", sanitize::sanitize_for_console(msg.as_ref()));
}

/// Returns a compact statusline string suitable for use as a rustyline prompt.
///
/// Format: `provider/model | fs_mode | basename ❯ `
pub fn statusline_prompt(
    provider: &str,
    model: &str,
    fs_mode: &str,
    cwd_basename: &str,
    turn: usize,
) -> String {
    format!(
        "{}/{} {} {} {} {} [{}] {} ",
        provider.cyan().bold(),
        model.cyan(),
        "│".dimmed(),
        fs_mode.yellow(),
        "│".dimmed(),
        cwd_basename.dimmed(),
        turn.to_string().dimmed(),
        "❯".purple().bold(),
    )
}

/// Returns a compact context block to prepend to outgoing user messages.
pub fn statusline_context(
    provider: &str,
    model: &str,
    fs_mode: &str,
    cwd: &str,
    turn: usize,
) -> String {
    format!(
        "<session_context provider=\"{provider}\" model=\"{model}\" fs_mode=\"{fs_mode}\" cwd=\"{cwd}\" turn=\"{turn}\" />\n"
    )
}

/// Two-line statusline prompt.
///
/// Line 1: `  dir <name> | git <branch> +<ahead> | chg <M>M <U>? | mdl <model> | ctx [████░░] N% Kt/Kt | $ N.NN`
/// Line 2: `  ❯ `
pub fn statusline_prompt_statusline(
    cwd_basename: &str,
    git: &crate::git_info::GitInfo,
    model: &str,
    ctx_tokens: u32,
    ctx_max: u32,
    session_cost: f64,
) -> String {
    // git segment
    let git_seg = if let Some(ref branch) = git.branch {
        let ahead = if git.ahead > 0 {
            format!(" +{}", git.ahead)
        } else {
            String::new()
        };
        format!("git {branch}{ahead}")
    } else {
        String::new()
    };

    // changes segment
    let chg_seg = {
        let mut parts = Vec::new();
        if git.modified > 0 {
            parts.push(format!("{}M", git.modified));
        }
        if git.untracked > 0 {
            parts.push(format!("{}?", git.untracked));
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("chg {}", parts.join(" "))
        }
    };

    // model badge — shorten e.g. "claude-sonnet-4-6" → "Sonnet 4.6"
    let mdl_seg = format!("mdl {}", shorten_model(model));

    // context bar
    let ctx_seg = if ctx_max > 0 {
        let pct = (ctx_tokens as f64 / ctx_max as f64).min(1.0);
        let filled = (pct * 15.0).round() as usize;
        let bar: String = (0..15)
            .map(|i| if i < filled { '█' } else { '░' })
            .collect();
        let pct_int = (pct * 100.0).round() as u32;
        let k_used = ctx_tokens / 1000;
        let k_max = ctx_max / 1000;
        format!("ctx [{bar}] {pct_int}% {k_used}K/{k_max}K")
    } else {
        String::new()
    };

    // cost
    let cost_seg = if session_cost > 0.0 {
        format!("$ {session_cost:.2}")
    } else {
        String::new()
    };

    // assemble non-empty segments
    let segments: Vec<String> = [
        Some(format!("dir {cwd_basename}")),
        if git_seg.is_empty() {
            None
        } else {
            Some(git_seg)
        },
        if chg_seg.is_empty() {
            None
        } else {
            Some(chg_seg)
        },
        Some(mdl_seg),
        if ctx_seg.is_empty() {
            None
        } else {
            Some(ctx_seg)
        },
        if cost_seg.is_empty() {
            None
        } else {
            Some(cost_seg)
        },
    ]
    .into_iter()
    .flatten()
    .collect();

    let line1 = format!("  {}", segments.join(" | "));
    format!("{}\n  {} ", line1.dimmed(), "❯".purple().bold())
}

fn shorten_model(model: &str) -> String {
    let m = model.to_lowercase();
    if m.contains("claude") {
        // "claude-sonnet-4-6" → "Sonnet 4.6", "claude-opus-4-8" → "Opus 4.8"
        let family = if m.contains("opus") {
            "Opus"
        } else if m.contains("haiku") {
            "Haiku"
        } else if m.contains("sonnet") {
            "Sonnet"
        } else {
            "Claude"
        };
        // extract trailing version digits e.g. "4-6" → "4.6"
        let version = model
            .split('-')
            .rev()
            .take(2)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(".");
        format!("{family} {version}")
    } else if m.starts_with("gemini") {
        // "gemini-2.0-flash" → "Gemini 2.0 Flash"
        model
            .split('-')
            .map(|s| {
                let mut c = s.chars();
                match c.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().to_string() + c.as_str(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        model.to_string()
    }
}

pub fn header(provider: &str, model: &str, cwd: &str) {
    // provider/model/cwd are not secrets typically, but treat as untrusted strings.
    let p = sanitize::sanitize_preview_for_console(provider);
    let m = sanitize::sanitize_preview_for_console(model);
    let d = sanitize::sanitize_preview_for_console(cwd);

    println!(
        "{} {} | {} | {}",
        ">>".bold(),
        "looprs".bold(),
        format!("{p}/{m}").cyan(),
        d.dimmed()
    );
    emit_machine_event(
        "header",
        serde_json::json!({
            "provider": provider,
            "model": model,
            "cwd": cwd,
        }),
    );
}

pub fn assistant_text(text: &str) {
    let safe = sanitize::sanitize_preview_for_console(text);
    println!("\n{} {}", "●".blue().bold(), safe.blue());
    emit_machine_event("assistant_text", serde_json::json!({ "text": text }));
}

/// Print the `●` lead-in for an assistant turn without any text, so
/// subsequent `write_chunk` calls can stream onto the same line.
pub fn assistant_lead_in() {
    print!("\n{} ", "●".blue().bold());
    let _ = std::io::Write::flush(&mut std::io::stdout());
}

/// Write one streamed chunk of assistant text in place, with no added
/// prefix or newline, flushing immediately so it renders incrementally.
pub fn write_chunk(text: &str) {
    let safe = sanitize::sanitize_preview_for_console(text);
    print!("{}", safe.blue());
    let _ = std::io::Write::flush(&mut std::io::stdout());
    emit_machine_event("write_chunk", serde_json::json!({ "text": text }));
}

pub fn tool_call(tool_name: &str, input_preview: &str) {
    let safe_name = sanitize::sanitize_preview_for_console(tool_name);
    let safe_preview = sanitize::sanitize_preview_for_console(input_preview);

    println!(
        "\n{} {}({})",
        "⚙".yellow().bold(),
        safe_name.yellow().bold(),
        safe_preview.dimmed()
    );
    emit_machine_event(
        "tool_call",
        serde_json::json!({
            "tool": tool_name,
            "preview": input_preview,
        }),
    );
}

pub fn tool_ok() {
    println!("  {} {}", "└─".green(), "OK".green());
    emit_machine_event("tool_ok", serde_json::json!({}));
}

pub fn tool_err(err_msg: &str) {
    let safe = sanitize::sanitize_preview_for_console(err_msg);
    println!("  {} {}", "└─".red(), safe.red());
    emit_machine_event("tool_err", serde_json::json!({ "error": err_msg }));
}

pub fn section_title(title: &str) {
    let safe = sanitize::sanitize_preview_for_console(title);
    println!("\n{}", safe.dimmed());
}

pub fn kv_preview(key: &str, value_preview: &str) {
    let k = sanitize::sanitize_preview_for_console(key);
    let v = sanitize::sanitize_preview_for_console(value_preview);
    println!("  {} {}", k.cyan(), v.dimmed());
}

pub fn running_command(command: &str) {
    let safe = sanitize::sanitize_preview_for_console(command);
    println!("{} Running: {}", "●".dimmed(), safe.dimmed());
    emit_machine_event("running_command", serde_json::json!({ "command": command }));
}

pub fn output_preview(text: &str) {
    let safe = sanitize::sanitize_preview_for_console(text);
    println!("{safe}");
}

/// Print raw output preserving ANSI color codes, then return a sanitized
/// (ANSI-stripped) copy for LLM context injection.
pub fn output_preview_colored(text: &str) -> String {
    let truncated = sanitize::truncate_preview(text);
    println!("{truncated}");
    sanitize::strip_ansi(&truncated)
}

pub fn goodbye() {
    println!("\n{}", "Goodbye!".dimmed());
    emit_machine_event("goodbye", serde_json::json!({}));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation_protocol::{
        ClockPort, EnvironmentPort, EventSequencePort, MACHINE_PROTOCOL_ENV, MACHINE_PROTOCOL_V1,
        RunIdentityPort,
    };
    use std::cell::Cell;
    use std::collections::HashMap;

    #[derive(Default)]
    struct FakeEnvironment(HashMap<String, String>);

    impl FakeEnvironment {
        fn with(mut self, name: &str, value: &str) -> Self {
            self.0.insert(name.to_string(), value.to_string());
            self
        }
    }

    impl EnvironmentPort for FakeEnvironment {
        fn var(&self, name: &str) -> Option<String> {
            self.0.get(name).cloned()
        }
    }

    struct FixedClock;

    impl ClockPort for FixedClock {
        fn epoch_millis(&self) -> u128 {
            1_000
        }

        fn rfc3339_utc(&self) -> String {
            "2026-01-01T00:00:00+00:00".to_string()
        }
    }

    struct FixedIdentity;

    impl RunIdentityPort for FixedIdentity {
        fn run_id(&self, _environment: &dyn EnvironmentPort, _clock: &dyn ClockPort) -> String {
            "ui-test".to_string()
        }
    }

    #[derive(Default)]
    struct LocalSequence(Cell<u64>);

    impl EventSequencePort for LocalSequence {
        fn next_sequence(&self) -> u64 {
            let next = self.0.get() + 1;
            self.0.set(next);
            next
        }
    }

    #[test]
    fn machine_event_writes_one_json_line_to_injected_sink() {
        let environment =
            FakeEnvironment::default().with(MACHINE_PROTOCOL_ENV, MACHINE_PROTOCOL_V1);
        let sequence = LocalSequence::default();
        let service = automation_protocol::AutomationProtocol::new(
            &environment,
            &FixedClock,
            &FixedIdentity,
            &sequence,
        );
        let mut output = Vec::new();

        let event = write_machine_event(
            &service,
            &mut output,
            "run.started",
            serde_json::json!({"ok": true}),
        )
        .expect("event should serialize")
        .expect("enabled protocol should emit an event");

        assert_eq!(output.iter().filter(|byte| **byte == b'\n').count(), 1);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output).unwrap(),
            event
        );
        assert_eq!(event["protocol"], MACHINE_PROTOCOL_V1);
        assert_eq!(event["run_id"], "ui-test");
        assert_eq!(event["event"]["kind"], "run.started");
    }

    #[test]
    fn machine_event_is_silent_when_injected_protocol_is_disabled() {
        let environment = FakeEnvironment::default();
        let sequence = LocalSequence::default();
        let service = automation_protocol::AutomationProtocol::new(
            &environment,
            &FixedClock,
            &FixedIdentity,
            &sequence,
        );
        let mut output = Vec::new();

        let event = write_machine_event(&service, &mut output, "ignored", serde_json::Value::Null)
            .expect("disabled output should succeed");

        assert!(event.is_none());
        assert!(output.is_empty());
    }
}
