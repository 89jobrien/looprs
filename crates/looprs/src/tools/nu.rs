use super::ToolArgs;
use super::error::ToolError;
use serde_json::Value;

// qual:allow(iosp) reason: "I/O boundary — parses args, runs nushell, returns output"
pub(super) fn tool_nu(args: &Value) -> Result<String, ToolError> {
    let args = ToolArgs::new(args);
    let cmd = args.get_str("cmd")?;

    let output = crate::shell::run_nu_command(cmd)?;

    let mut result = String::from_utf8_lossy(&output.stdout).to_string();

    if !output.stderr.is_empty() {
        result.push_str("\n--- stderr ---\n");
        result.push_str(&String::from_utf8_lossy(&output.stderr));
    }

    if !output.status.success() {
        return Err(ToolError::CommandFailed(format!(
            "Exit code: {}",
            output.status.code().unwrap_or(-1)
        )));
    }

    Ok(if result.trim().is_empty() {
        "(empty)".to_string()
    } else {
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nu_runs_command() {
        let args = json!({"cmd": "\"ok\""});
        let out = tool_nu(&args).expect("nu tool should run");
        assert!(out.contains("ok"));
    }
}
