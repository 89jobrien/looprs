use super::ToolArgs;
use super::error::ToolError;
use serde_json::Value;

/// Hard cap on raw bytes collected from a single tool invocation.
const MAX_OUTPUT_BYTES: usize = 512 * 1024; // 512 KiB

// qual:allow(iosp) reason: "I/O boundary — parses args, runs nushell, returns output"
pub(super) fn tool_nu(args: &Value) -> Result<String, ToolError> {
    let args = ToolArgs::new(args);
    let cmd = args.get_str("cmd")?;

    let output = crate::shell::run_nu_command(cmd)?;

    let stdout = truncate_bytes(&output.stdout, MAX_OUTPUT_BYTES);
    let mut result = String::from_utf8_lossy(stdout).to_string();

    if !output.stderr.is_empty() {
        result.push_str("\n--- stderr ---\n");
        result.push_str(&String::from_utf8_lossy(truncate_bytes(
            &output.stderr,
            MAX_OUTPUT_BYTES,
        )));
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

fn truncate_bytes(data: &[u8], max: usize) -> &[u8] {
    if data.len() <= max {
        return data;
    }
    let mut end = max;
    while end > 0 && (data[end] & 0xC0) == 0x80 {
        end -= 1;
    }
    &data[..end]
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
