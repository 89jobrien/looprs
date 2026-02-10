use colored::*;

use crate::sanitize;

/// Environment variable that enables machine-readable JSON logs when set to "1" or "true".
const MACHINE_LOG_ENV: &str = "LOOPRS_MACHINE_LOG";

pub fn init_logging() {
    // C2a: internal logs are opt-in via RUST_LOG. UI output remains separate.
    let mut builder = env_logger::Builder::from_default_env();
    // If user hasn't set RUST_LOG, default to warnings+.
    if std::env::var("RUST_LOG").is_err() {
        builder.filter_level(log::LevelFilter::Warn);
    }
    let _ = builder.try_init();
}

fn machine_log_enabled() -> bool {
    matches!(
        std::env::var(MACHINE_LOG_ENV)
            .ok()
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("1") | Some("true")
    )
}

fn emit_machine_event(kind: &str, data: serde_json::Value) {
    if !machine_log_enabled() {
        return;
    }

    let event = serde_json::json!({
        "kind": kind,
        "data": data,
    });

    if let Ok(line) = serde_json::to_string(&event) {
        eprintln!("{line}");
    }
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

pub fn warn_full(msg: impl AsRef<str>) {
    eprintln!("{}", sanitize::sanitize_for_console(msg.as_ref()));
}

pub fn error(msg: impl AsRef<str>) {
    let raw = msg.as_ref();
    eprintln!("{}", sanitize::sanitize_preview_for_console(raw));
    emit_machine_event("error", serde_json::json!({ "message": raw }));
}

pub fn error_full(msg: impl AsRef<str>) {
    eprintln!("{}", sanitize::sanitize_for_console(msg.as_ref()));
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

pub fn goodbye() {
    println!("\n{}", "Goodbye!".dimmed());
    emit_machine_event("goodbye", serde_json::json!({}));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn machine_log_disabled_by_default() {
        // SAFETY: test-only environment mutation.
        unsafe {
            std::env::remove_var(MACHINE_LOG_ENV);
        }
        assert!(!machine_log_enabled());
    }

    #[test]
    fn machine_log_enabled_with_true_like_values() {
        for v in &["1", "true", "True", "TRUE"] {
            // SAFETY: test-only environment mutation.
            unsafe {
                std::env::set_var(MACHINE_LOG_ENV, v);
            }
            assert!(machine_log_enabled(), "value {v} should enable machine log");
        }
    }
}
