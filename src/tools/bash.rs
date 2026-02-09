use super::error::ToolError;
use super::ToolArgs;
use serde_json::Value;
use std::process::{Command, Stdio};

pub(super) fn tool_bash(args: &Value) -> Result<String, ToolError> {
    let args = ToolArgs::new(args);
    let cmd = args.get_str("cmd")?;

    let output = Command::new("bash")
        .arg("-c")
        .arg(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

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
    fn bash_runs_command() {
        let args = json!({"cmd": "echo ok"});
        let out = tool_bash(&args).unwrap();
        assert!(out.contains("ok"));
    }
}
