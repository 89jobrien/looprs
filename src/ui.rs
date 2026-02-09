use colored::*;

use crate::sanitize;

pub fn init_logging() {
    // C2a: internal logs are opt-in via RUST_LOG. UI output remains separate.
    let mut builder = env_logger::Builder::from_default_env();
    // If user hasn't set RUST_LOG, default to warnings+.
    if std::env::var("RUST_LOG").is_err() {
        builder.filter_level(log::LevelFilter::Warn);
    }
    let _ = builder.try_init();
}

pub fn info(msg: impl AsRef<str>) {
    println!("{}", sanitize::sanitize_preview_for_console(msg.as_ref()));
}

pub fn info_full(msg: impl AsRef<str>) {
    println!("{}", sanitize::sanitize_for_console(msg.as_ref()));
}

pub fn warn(msg: impl AsRef<str>) {
    eprintln!("{}", sanitize::sanitize_preview_for_console(msg.as_ref()));
}

pub fn warn_full(msg: impl AsRef<str>) {
    eprintln!("{}", sanitize::sanitize_for_console(msg.as_ref()));
}

pub fn error(msg: impl AsRef<str>) {
    eprintln!("{}", sanitize::sanitize_preview_for_console(msg.as_ref()));
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
}

pub fn assistant_text(text: &str) {
    let safe = sanitize::sanitize_preview_for_console(text);
    println!("\n{} {}", "●".blue().bold(), safe.blue());
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
}

pub fn tool_ok() {
    println!("  {} {}", "└─".green(), "OK".green());
}

pub fn tool_err(err_msg: &str) {
    let safe = sanitize::sanitize_preview_for_console(err_msg);
    println!("  {} {}", "└─".red(), safe.red());
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
}

pub fn output_preview(text: &str) {
    let safe = sanitize::sanitize_preview_for_console(text);
    println!("{safe}");
}

pub fn goodbye() {
    println!("\n{}", "Goodbye!".dimmed());
}
