use anyhow::{Result, anyhow};
use looprs::automation_protocol::{MACHINE_PROTOCOL_V1, MachineProtocol};
use std::env;

#[derive(Debug, Clone)]
pub struct CliArgs {
    pub prompt: Option<String>,           // -p/--prompt
    pub file: Option<String>,             // -f/--file
    pub model: Option<String>,            // -m/--model
    pub quiet: bool,                      // -q/--quiet
    pub no_hooks: bool,                   // --no-hooks
    pub json_output: bool,                // --json
    pub machine_log: bool,                // --machine-log
    pub machine_protocol: Option<String>, // --machine-protocol
    pub run_id: Option<String>,           // --run-id
    pub deadline_seconds: Option<u64>,    // --deadline-seconds
    pub cancel_file: Option<String>,      // --cancel-file
}

impl CliArgs {
    /// Parse command-line arguments
    pub fn parse() -> Result<Self> {
        let args: Vec<String> = env::args().collect();
        Self::parse_from(&args[1..])
    }

    /// Parse from a slice of arguments (for testing)
    #[allow(dead_code)]
    pub fn parse_from(args: &[String]) -> Result<Self> {
        let mut result = CliArgs {
            prompt: None,
            file: None,
            model: None,
            quiet: false,
            no_hooks: false,
            json_output: false,
            machine_log: false,
            machine_protocol: None,
            run_id: None,
            deadline_seconds: None,
            cancel_file: None,
        };

        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];

            match arg.as_str() {
                "-p" | "--prompt" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow!("{arg} requires a value"));
                    }
                    result.prompt = Some(args[i].clone());
                }
                "-f" | "--file" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow!("{arg} requires a value"));
                    }
                    result.file = Some(args[i].clone());
                }
                "-m" | "--model" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow!("{arg} requires a value"));
                    }
                    result.model = Some(args[i].clone());
                }
                "-q" | "--quiet" => {
                    result.quiet = true;
                }
                "--no-hooks" => {
                    result.no_hooks = true;
                }
                "--json" => {
                    result.json_output = true;
                }
                "--machine-log" => {
                    if result.machine_log {
                        return Err(anyhow!("{arg} specified more than once"));
                    }
                    result.machine_log = true;
                }
                "--machine-protocol" => {
                    if result.machine_protocol.is_some() {
                        return Err(anyhow!("{arg} specified more than once"));
                    }
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow!("{arg} requires a value"));
                    }
                    result.machine_protocol = Some(args[i].clone());
                }
                "--run-id" => {
                    if result.run_id.is_some() {
                        return Err(anyhow!("{arg} specified more than once"));
                    }
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow!("{arg} requires a value"));
                    }
                    result.run_id = Some(args[i].clone());
                }
                "--deadline-seconds" => {
                    if result.deadline_seconds.is_some() {
                        return Err(anyhow!("{arg} specified more than once"));
                    }
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow!("{arg} requires a value"));
                    }
                    let parsed = args[i]
                        .parse::<u64>()
                        .map_err(|_| anyhow!("{arg} must be an integer number of seconds"))?;
                    result.deadline_seconds = Some(parsed);
                }
                "--cancel-file" => {
                    if result.cancel_file.is_some() {
                        return Err(anyhow!("{arg} specified more than once"));
                    }
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow!("{arg} requires a value"));
                    }
                    result.cancel_file = Some(args[i].clone());
                }
                unknown => {
                    return Err(anyhow!("Unknown argument: {unknown}"));
                }
            }

            i += 1;
        }

        if let Some(protocol) = result.machine_protocol.as_mut() {
            protocol.parse::<MachineProtocol>().map_err(|_| {
                anyhow!(
                    "Unsupported machine protocol '{protocol}', expected '{MACHINE_PROTOCOL_V1}'"
                )
            })?;
            *protocol = protocol.trim().to_string();
        }

        let has_run_controls = result.run_id.is_some()
            || result.deadline_seconds.is_some()
            || result.cancel_file.is_some();
        if has_run_controls && result.machine_protocol.is_none() {
            return Err(anyhow!(
                "--machine-protocol is required when using --run-id, --deadline-seconds, or --cancel-file"
            ));
        }

        if result
            .run_id
            .as_deref()
            .is_some_and(|run_id| run_id.trim().is_empty())
        {
            return Err(anyhow!("--run-id cannot be empty"));
        }

        if result
            .deadline_seconds
            .is_some_and(|deadline| deadline == 0)
        {
            return Err(anyhow!("--deadline-seconds must be greater than 0"));
        }

        if result
            .cancel_file
            .as_deref()
            .is_some_and(|path| path.trim().is_empty())
        {
            return Err(anyhow!("--cancel-file cannot be empty"));
        }

        Ok(result)
    }

    /// Determine if running in scriptable (non-interactive) mode
    pub fn is_scriptable(&self) -> bool {
        self.prompt.is_some() || self.file.is_some()
    }

    /// Read prompt from file if specified, or use inline prompt
    pub fn get_prompt(&self) -> Result<Option<String>> {
        if let Some(ref file_path) = self.file {
            let content = std::fs::read_to_string(file_path)
                .map_err(|e| anyhow!("Failed to read file {file_path}: {e}"))?;
            Ok(Some(content.trim().to_string()))
        } else if let Some(ref prompt) = self.prompt {
            Ok(Some(prompt.clone()))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_no_args() {
        let parsed = CliArgs::parse_from(&args(&[])).unwrap();
        assert!(parsed.prompt.is_none());
        assert!(parsed.file.is_none());
        assert!(parsed.model.is_none());
        assert!(!parsed.quiet);
        assert!(!parsed.no_hooks);
        assert!(!parsed.json_output);
        assert!(!parsed.machine_log);
        assert!(parsed.machine_protocol.is_none());
        assert!(parsed.run_id.is_none());
        assert!(parsed.deadline_seconds.is_none());
        assert!(parsed.cancel_file.is_none());
    }

    #[test]
    fn parse_prompt_short() {
        let parsed = CliArgs::parse_from(&args(&["-p", "hello world"])).unwrap();
        assert_eq!(parsed.prompt, Some("hello world".to_string()));
        assert!(!parsed.quiet);
    }

    #[test]
    fn parse_prompt_long() {
        let parsed = CliArgs::parse_from(&args(&["--prompt", "hello world"])).unwrap();
        assert_eq!(parsed.prompt, Some("hello world".to_string()));
    }

    #[test]
    fn parse_file_short() {
        let parsed = CliArgs::parse_from(&args(&["-f", "test.txt"])).unwrap();
        assert_eq!(parsed.file, Some("test.txt".to_string()));
    }

    #[test]
    fn parse_file_long() {
        let parsed = CliArgs::parse_from(&args(&["--file", "test.txt"])).unwrap();
        assert_eq!(parsed.file, Some("test.txt".to_string()));
    }

    #[test]
    fn parse_model_short() {
        let parsed = CliArgs::parse_from(&args(&["-m", "gpt-5.2"])).unwrap();
        assert_eq!(parsed.model, Some("gpt-5.2".to_string()));
    }

    #[test]
    fn parse_model_long() {
        let parsed = CliArgs::parse_from(&args(&["--model", "claude-3-opus"])).unwrap();
        assert_eq!(parsed.model, Some("claude-3-opus".to_string()));
    }

    #[test]
    fn parse_quiet_short() {
        let parsed = CliArgs::parse_from(&args(&["-q"])).unwrap();
        assert!(parsed.quiet);
    }

    #[test]
    fn parse_quiet_long() {
        let parsed = CliArgs::parse_from(&args(&["--quiet"])).unwrap();
        assert!(parsed.quiet);
    }

    #[test]
    fn parse_no_hooks() {
        let parsed = CliArgs::parse_from(&args(&["--no-hooks"])).unwrap();
        assert!(parsed.no_hooks);
    }

    #[test]
    fn parse_json() {
        let parsed = CliArgs::parse_from(&args(&["--json"])).unwrap();
        assert!(parsed.json_output);
        assert!(!parsed.machine_log);
    }

    #[test]
    fn parse_machine_log() {
        let parsed = CliArgs::parse_from(&args(&["--machine-log"])).unwrap();
        assert!(parsed.machine_log);
        assert!(!parsed.json_output);
    }

    #[test]
    fn parse_machine_protocol_with_run_controls() {
        let parsed = CliArgs::parse_from(&args(&[
            "--machine-protocol",
            MACHINE_PROTOCOL_V1,
            "--run-id",
            "run-123",
            "--deadline-seconds",
            "30",
            "--cancel-file",
            "/tmp/looprs.cancel",
        ]))
        .unwrap();

        assert_eq!(
            parsed.machine_protocol.as_deref(),
            Some(MACHINE_PROTOCOL_V1)
        );
        assert_eq!(parsed.run_id.as_deref(), Some("run-123"));
        assert_eq!(parsed.deadline_seconds, Some(30));
        assert_eq!(parsed.cancel_file.as_deref(), Some("/tmp/looprs.cancel"));
    }

    #[test]
    fn parse_machine_protocol_rejects_unknown_version() {
        let result = CliArgs::parse_from(&args(&["--machine-protocol", "v2"]));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unsupported machine protocol")
        );
    }

    #[test]
    fn parse_deadline_rejects_zero() {
        let result = CliArgs::parse_from(&args(&[
            "--machine-protocol",
            MACHINE_PROTOCOL_V1,
            "--deadline-seconds",
            "0",
        ]));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("must be greater than 0")
        );
    }

    #[test]
    fn run_controls_require_machine_protocol() {
        let run_id_only = CliArgs::parse_from(&args(&["--run-id", "run-123"]));
        assert!(run_id_only.is_err());
        assert!(
            run_id_only
                .unwrap_err()
                .to_string()
                .contains("--machine-protocol")
        );

        let deadline_only = CliArgs::parse_from(&args(&["--deadline-seconds", "30"]));
        assert!(deadline_only.is_err());
        assert!(
            deadline_only
                .unwrap_err()
                .to_string()
                .contains("--machine-protocol")
        );

        let cancel_file_only = CliArgs::parse_from(&args(&["--cancel-file", "/tmp/looprs.cancel"]));
        assert!(cancel_file_only.is_err());
        assert!(
            cancel_file_only
                .unwrap_err()
                .to_string()
                .contains("--machine-protocol")
        );
    }

    #[test]
    fn run_controls_with_unknown_protocol_fail_with_protocol_error() {
        let result =
            CliArgs::parse_from(&args(&["--machine-protocol", "v2", "--run-id", "run-123"]));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Unsupported machine protocol")
        );
    }

    #[test]
    fn run_controls_with_protocol_still_validate_control_values() {
        let result = CliArgs::parse_from(&args(&[
            "--machine-protocol",
            MACHINE_PROTOCOL_V1,
            "--run-id",
            "  ",
        ]));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("--run-id cannot be empty")
        );
    }

    #[test]
    fn machine_options_reject_missing_values() {
        for option in [
            "--machine-protocol",
            "--run-id",
            "--deadline-seconds",
            "--cancel-file",
        ] {
            let result = CliArgs::parse_from(&args(&[option]));
            assert!(result.is_err(), "{option} should require a value");
            assert!(result.unwrap_err().to_string().contains("requires a value"));
        }
    }

    #[test]
    fn deadline_rejects_negative_nonnumeric_and_overflow_values() {
        for value in ["-1", "soon", "18446744073709551616"] {
            let result = CliArgs::parse_from(&args(&[
                "--machine-protocol",
                MACHINE_PROTOCOL_V1,
                "--deadline-seconds",
                value,
            ]));
            assert!(result.is_err(), "deadline {value:?} should fail");
        }
    }

    #[test]
    fn cancel_file_rejects_empty_path() {
        let result = CliArgs::parse_from(&args(&[
            "--machine-protocol",
            MACHINE_PROTOCOL_V1,
            "--cancel-file",
            "  ",
        ]));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[test]
    fn machine_options_reject_duplicates() {
        let cases = [
            vec!["--machine-log", "--machine-log"],
            vec![
                "--machine-protocol",
                MACHINE_PROTOCOL_V1,
                "--machine-protocol",
                MACHINE_PROTOCOL_V1,
            ],
            vec![
                "--machine-protocol",
                MACHINE_PROTOCOL_V1,
                "--run-id",
                "one",
                "--run-id",
                "two",
            ],
            vec![
                "--machine-protocol",
                MACHINE_PROTOCOL_V1,
                "--deadline-seconds",
                "1",
                "--deadline-seconds",
                "2",
            ],
            vec![
                "--machine-protocol",
                MACHINE_PROTOCOL_V1,
                "--cancel-file",
                "one",
                "--cancel-file",
                "two",
            ],
        ];
        for case in cases {
            let result = CliArgs::parse_from(&args(&case));
            assert!(result.is_err(), "duplicate case {case:?} should fail");
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("specified more than once")
            );
        }
    }

    #[test]
    fn machine_log_does_not_enable_versioned_run_controls() {
        let result = CliArgs::parse_from(&args(&["--machine-log", "--run-id", "run-123"]));
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("--machine-protocol")
        );
    }

    #[test]
    fn parse_combined_args() {
        let parsed = CliArgs::parse_from(&args(&[
            "-p",
            "generate code",
            "-m",
            "gpt-5.2-codex",
            "-q",
            "--json",
            "--machine-log",
        ]))
        .unwrap();

        assert_eq!(parsed.prompt, Some("generate code".to_string()));
        assert_eq!(parsed.model, Some("gpt-5.2-codex".to_string()));
        assert!(parsed.quiet);
        assert!(parsed.json_output);
        assert!(parsed.machine_log);
        assert!(!parsed.no_hooks);
    }

    #[test]
    fn parse_all_args() {
        let parsed = CliArgs::parse_from(&args(&[
            "-p",
            "fix this",
            "-m",
            "claude-3-opus",
            "-q",
            "--no-hooks",
            "--json",
            "--machine-log",
        ]))
        .unwrap();

        assert_eq!(parsed.prompt, Some("fix this".to_string()));
        assert_eq!(parsed.model, Some("claude-3-opus".to_string()));
        assert!(parsed.quiet);
        assert!(parsed.no_hooks);
        assert!(parsed.json_output);
        assert!(parsed.machine_log);
    }

    #[test]
    fn parse_error_on_unknown_arg() {
        let result = CliArgs::parse_from(&args(&["--unknown"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown argument"));
    }

    #[test]
    fn parse_error_on_missing_prompt_value() {
        let result = CliArgs::parse_from(&args(&["-p"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("requires a value"));
    }

    #[test]
    fn parse_error_on_missing_file_value() {
        let result = CliArgs::parse_from(&args(&["-f"]));
        assert!(result.is_err());
    }

    #[test]
    fn parse_error_on_missing_model_value() {
        let result = CliArgs::parse_from(&args(&["-m"]));
        assert!(result.is_err());
    }

    #[test]
    fn is_scriptable_with_prompt() {
        let parsed = CliArgs::parse_from(&args(&["-p", "hello"])).unwrap();
        assert!(parsed.is_scriptable());
    }

    #[test]
    fn is_scriptable_with_file() {
        let parsed = CliArgs::parse_from(&args(&["-f", "test.txt"])).unwrap();
        assert!(parsed.is_scriptable());
    }

    #[test]
    fn is_scriptable_with_both() {
        let parsed = CliArgs::parse_from(&args(&["-p", "hello", "-f", "test.txt"])).unwrap();
        assert!(parsed.is_scriptable());
    }

    #[test]
    fn not_scriptable_without_prompt_or_file() {
        let parsed = CliArgs::parse_from(&args(&["-q", "--json"])).unwrap();
        assert!(!parsed.is_scriptable());
    }

    #[test]
    fn get_prompt_from_option() {
        let parsed = CliArgs::parse_from(&args(&["-p", "hello world"])).unwrap();
        let prompt = parsed.get_prompt().unwrap();
        assert_eq!(prompt, Some("hello world".to_string()));
    }

    #[test]
    fn get_prompt_none() {
        let parsed = CliArgs::parse_from(&args(&["-q"])).unwrap();
        let prompt = parsed.get_prompt().unwrap();
        assert_eq!(prompt, None);
    }

    #[test]
    fn file_arg_overrides_in_get_prompt() {
        // When both are provided, file takes precedence in get_prompt
        // This test will be updated once we handle file reading
        let parsed =
            CliArgs::parse_from(&args(&["-p", "inline", "-f", "nonexistent.txt"])).unwrap();
        // Attempt to get prompt will fail because file doesn't exist
        let result = parsed.get_prompt();
        assert!(result.is_err());
    }
}
