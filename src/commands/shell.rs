use anyhow::Result;
use crate::commands::CommandResult;
use crate::session::Session;

/// Shell passthrough command
#[derive(Debug, Clone)]
pub struct ShellPassthrough {
    pub command: String,
}

/// Execute shell command in the isolated sandbox
pub async fn execute_shell_command(cmd: ShellPassthrough, session: &mut Session) -> Result<CommandResult> {
    tracing::info!("🔒 Executing shell command in sandbox: {}", cmd.command);

    let output = session.execute_shell_command(&cmd.command).await?;

    let result_msg = format!(
        "Shell Command: {}\n\nOutput:\n{}",
        cmd.command,
        output
    );

    Ok(CommandResult::Message(result_msg))
}
