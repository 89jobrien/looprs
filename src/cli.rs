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
