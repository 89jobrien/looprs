pub enum CliCommand {
    Quit,
    Clear,
    Message(String),
}

pub fn parse_input(line: &str) -> Option<CliCommand> {
    let trimmed = line.trim();

    if trimmed.is_empty() {
        return None;
    }

    match trimmed {
        "/q" | "exit" | "quit" => Some(CliCommand::Quit),
        "/c" | "clear" => Some(CliCommand::Clear),
        msg => Some(CliCommand::Message(msg.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::parse_input;
    use super::CliCommand;

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
        assert!(matches!(
            parse_input("hello"),
            Some(CliCommand::Message(_))
        ));
    }

    #[test]
    fn ignore_empty_input() {
        assert!(parse_input("").is_none());
        assert!(parse_input("   ").is_none());
    }
}
