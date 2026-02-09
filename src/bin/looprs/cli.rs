pub enum CliCommand {
    Quit,
    Clear,
    CustomCommand(String), // Custom command from .looprs/commands/
    InvokeSkill(String),   // Explicit skill invocation: $skill-name
    Message(String),
}

pub fn parse_input(line: &str) -> Option<CliCommand> {
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return None;
    }

    // Check for explicit skill invocation ($ prefix)
    if trimmed.starts_with('$') && trimmed.len() > 1 {
        let skill_name = trimmed[1..].split_whitespace().next().unwrap_or("");
        if !skill_name.is_empty() {
            return Some(CliCommand::InvokeSkill(skill_name.to_string()));
        }
    }

    // Check for custom commands (/ prefix)
    if trimmed.starts_with('/') && trimmed.len() > 1 {
        let command_name = trimmed[1..].split_whitespace().next().unwrap_or("");
        if !command_name.is_empty() && command_name != "q" && command_name != "c" {
            return Some(CliCommand::CustomCommand(trimmed[1..].to_string()));
        }
    }

    match trimmed {
        "/q" | "exit" | "quit" => Some(CliCommand::Quit),
        "/c" | "clear" => Some(CliCommand::Clear),
        msg => Some(CliCommand::Message(msg.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::CliCommand;
    use super::parse_input;

    #[test]
    fn parse_quit_commands() {
        assert!(matches!(parse_input("/q"), Some(CliCommand::Quit)));
        assert!(matches!(parse_input("exit"), Some(CliCommand::Quit)));
        assert!(matches!(parse_input("quit"), Some(CliCommand::Quit)));
    }

    #[test]
    fn parse_clear_commands() {
        assert!(matches!(parse_input("/c"), Some(CliCommand::Clear)));
        assert!(matches!(parse_input("clear"), Some(CliCommand::Clear)));
    }

    #[test]
    fn parse_message_commands() {
        assert!(matches!(parse_input("hello"), Some(CliCommand::Message(_))));
    }

    #[test]
    fn parse_custom_commands() {
        assert!(matches!(
            parse_input("/refactor"),
            Some(CliCommand::CustomCommand(_))
        ));
        assert!(matches!(
            parse_input("/lint --fix"),
            Some(CliCommand::CustomCommand(_))
        ));
    }

    #[test]
    fn ignore_empty_input() {
        assert!(parse_input("").is_none());
        assert!(parse_input("   ").is_none());
    }

    #[test]
    fn parse_skill_invocation() {
        assert!(matches!(
            parse_input("$rust-testing"),
            Some(CliCommand::InvokeSkill(_))
        ));
        
        if let Some(CliCommand::InvokeSkill(name)) = parse_input("$rust-testing") {
            assert_eq!(name, "rust-testing");
        }
    }

    #[test]
    fn parse_skill_invocation_ignores_trailing_text() {
        if let Some(CliCommand::InvokeSkill(name)) = parse_input("$rust-testing help me") {
            assert_eq!(name, "rust-testing");
        }
    }

    #[test]
    fn parse_empty_skill_name() {
        // Just "$" should be treated as a message
        assert!(matches!(parse_input("$"), Some(CliCommand::Message(_))));
    }
}
