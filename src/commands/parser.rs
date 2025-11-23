use super::{SlashCommand, AtCommand, ShellPassthrough};

/// Parsed command types
#[derive(Debug, Clone)]
pub enum ParsedCommand {
    /// Slash command (/help, /quit, etc.)
    Slash(SlashCommand),
    /// At-command for context attachment (@file.rs)
    AtCommand(AtCommand),
    /// Shell passthrough (!ls -la)
    ShellPassthrough(ShellPassthrough),
    /// Regular user message
    Regular(String),
}

/// Command parser
pub struct CommandParser;

impl CommandParser {
    /// Parse user input into a command
    pub fn parse(input: &str) -> ParsedCommand {
        let trimmed = input.trim();

        if trimmed.is_empty() {
            return ParsedCommand::Regular(input.to_string());
        }

        // Check for slash commands
        if trimmed.starts_with('/') {
            return ParsedCommand::Slash(SlashCommand::parse(trimmed));
        }

        // Check for shell passthrough
        if trimmed.starts_with('!') {
            let command = trimmed[1..].trim().to_string();
            return ParsedCommand::ShellPassthrough(ShellPassthrough { command });
        }

        // Check for at-commands in the input
        if trimmed.contains('@') {
            match AtCommand::parse(trimmed) {
                Some(at_cmd) => return ParsedCommand::AtCommand(at_cmd),
                None => {} // Fall through to regular message
            }
        }

        // Regular message
        ParsedCommand::Regular(input.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_slash_command() {
        let cmd = CommandParser::parse("/help");
        assert!(matches!(cmd, ParsedCommand::Slash(_)));
    }

    #[test]
    fn test_parse_shell_passthrough() {
        let cmd = CommandParser::parse("!ls -la");
        assert!(matches!(cmd, ParsedCommand::ShellPassthrough(_)));
    }

    #[test]
    fn test_parse_regular() {
        let cmd = CommandParser::parse("Hello world");
        assert!(matches!(cmd, ParsedCommand::Regular(_)));
    }
}
