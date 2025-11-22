mod parser;
mod slash;
mod at_command;
mod shell;

pub use parser::{CommandParser, ParsedCommand};
pub use slash::SlashCommand;
pub use at_command::AtCommand;
pub use shell::ShellPassthrough;

use anyhow::Result;
use crate::session::Session;

/// Command execution result
#[derive(Debug, Clone)]
pub enum CommandResult {
    /// Continue the session normally
    Continue,
    /// Display a message to the user
    Message(String),
    /// Exit the session
    Exit,
    /// Clear the screen
    Clear,
    /// Execute with modified input
    ModifiedInput(String),
}

/// Execute a parsed command
pub async fn execute_command(cmd: ParsedCommand, session: &mut Session) -> Result<CommandResult> {
    match cmd {
        ParsedCommand::Slash(slash_cmd) => slash::execute_slash_command(slash_cmd, session).await,
        ParsedCommand::AtCommand(at_cmd) => at_command::execute_at_command(at_cmd, session).await,
        ParsedCommand::ShellPassthrough(shell_cmd) => shell::execute_shell_command(shell_cmd, session).await,
        ParsedCommand::Regular(text) => Ok(CommandResult::ModifiedInput(text)),
    }
}
