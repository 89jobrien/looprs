use anyhow::{Result, anyhow};
use std::env;

#[derive(Debug, Clone)]
pub struct CliArgs {
    pub prompt: Option<String>, // -p/--prompt
    pub file: Option<String>,   // -f/--file
    pub model: Option<String>,  // -m/--model
    pub quiet: bool,            // -q/--quiet
    pub no_hooks: bool,         // --no-hooks
    pub json_output: bool,      // --json
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
                unknown => {
                    return Err(anyhow!("Unknown argument: {unknown}"));
                }
            }

            i += 1;
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
        ]))
        .unwrap();

        assert_eq!(parsed.prompt, Some("generate code".to_string()));
        assert_eq!(parsed.model, Some("gpt-5.2-codex".to_string()));
        assert!(parsed.quiet);
        assert!(parsed.json_output);
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
        ]))
        .unwrap();

        assert_eq!(parsed.prompt, Some("fix this".to_string()));
        assert_eq!(parsed.model, Some("claude-3-opus".to_string()));
        assert!(parsed.quiet);
        assert!(parsed.no_hooks);
        assert!(parsed.json_output);
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
