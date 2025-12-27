//! Shell TUI runner - event loop and command execution
//!
//! This module handles the main event loop for the shell-first TUI,
//! including keyboard input, command execution, and AI integration.

use anyhow::{Context, Result};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader as TokioBufReader};
use tokio::process::Command as TokioCommand;
use tokio::sync::{mpsc, Mutex};

use super::shell_app::{AiCommand, BlockOutput, BlockType, CommandBlock, ShellTuiApp};
use super::shell_ui;
use crate::config::Config;
use crate::orchestrator::{Orchestrator, OrchestratorConfig};
use crate::session::Session;

/// Message types for async command execution
#[derive(Debug)]
enum CommandUpdate {
    /// Append output line to block
    Output { block_id: String, line: String },
    /// Command completed
    Complete { block_id: String, exit_code: i32 },
    /// Command failed
    Failed {
        block_id: String,
        message: String,
        stderr: String,
        exit_code: i32,
    },
}

/// Message types for AI updates
#[derive(Debug)]
enum AiUpdate {
    /// AI response text
    Response { block_id: String, text: String },
    /// AI started tool execution
    ToolStart {
        parent_block_id: String,
        tool_name: String,
        description: String,
    },
    /// AI tool completed
    ToolComplete {
        parent_block_id: String,
        tool_id: String,
        output: String,
    },
    /// AI processing complete
    Complete { block_id: String },
    /// AI error
    Error { block_id: String, message: String },
}

/// Shell TUI runner
pub struct ShellTuiRunner {
    app: ShellTuiApp,
    config: Config,
}

impl ShellTuiRunner {
    /// Create a new shell TUI runner
    pub fn new(project_path: PathBuf, config: Config) -> Self {
        let app = ShellTuiApp::new(project_path, config.clone());
        Self { app, config }
    }

    /// Run the shell TUI
    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Run the event loop
        let result = self.run_event_loop(&mut terminal).await;

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    /// Run with auto-connect to AI
    pub async fn run_with_ai(&mut self) -> Result<()> {
        // Connect to AI first
        self.connect_ai().await?;
        self.run().await
    }

    /// Main event loop
    async fn run_event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    ) -> Result<()> {
        // Channels for async command execution
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<CommandUpdate>();
        let (ai_tx, mut ai_rx) = mpsc::unbounded_channel::<AiUpdate>();

        loop {
            // Draw if needed
            if self.app.needs_redraw {
                terminal.draw(|f| shell_ui::draw(f, &mut self.app))?;
                self.app.clear_dirty();
            }

            // Poll for events
            if event::poll(Duration::from_millis(30))? {
                if let Event::Key(key) = event::read()? {
                    match self
                        .handle_key_event(key.code, key.modifiers, &cmd_tx, &ai_tx)
                        .await
                    {
                        Ok(true) => break, // Exit requested
                        Ok(false) => {}
                        Err(e) => {
                            // Show error in UI
                            let prompt = self.app.current_prompt();
                            let mut block = CommandBlock::system(format!("Error: {}", e), prompt);
                            self.app.add_block(block);
                        }
                    }
                }
            }

            // Process command updates
            while let Ok(update) = cmd_rx.try_recv() {
                match update {
                    CommandUpdate::Output { block_id, line } => {
                        self.app.append_to_block(&block_id, line);
                    }
                    CommandUpdate::Complete {
                        block_id,
                        exit_code,
                    } => {
                        if let Some(block) = self.app.get_block_mut(&block_id) {
                            block.complete_streaming(exit_code);
                            self.app.last_exit_code = exit_code;
                        }
                        self.app.mark_dirty();
                    }
                    CommandUpdate::Failed {
                        block_id,
                        message,
                        stderr,
                        exit_code,
                    } => {
                        self.app.fail_block(&block_id, message, stderr, exit_code);
                    }
                }
            }

            // Process AI updates
            while let Ok(update) = ai_rx.try_recv() {
                match update {
                    AiUpdate::Response { block_id, text } => {
                        if let Some(block) = self.app.get_block_mut(&block_id) {
                            block.output = BlockOutput::Success(text);
                        }
                        self.app.mark_dirty();
                    }
                    AiUpdate::ToolStart {
                        parent_block_id,
                        tool_name,
                        description,
                    } => {
                        // Get prompt first before mutable borrow
                        let prompt = self.app.current_prompt();
                        let child = CommandBlock::new(
                            description,
                            BlockType::AiToolExecution {
                                tool_name: tool_name.clone(),
                            },
                            prompt,
                        );
                        if let Some(parent) = self.app.get_block_mut(&parent_block_id) {
                            parent.add_child(child);
                        }
                        self.app.mark_dirty();
                    }
                    AiUpdate::ToolComplete {
                        parent_block_id: _,
                        tool_id: _,
                        output: _,
                    } => {
                        // Tool completion handled - just mark dirty
                        self.app.mark_dirty();
                    }
                    AiUpdate::Complete { block_id } => {
                        if let Some(block) = self.app.get_block_mut(&block_id) {
                            if matches!(block.output, BlockOutput::Pending) {
                                block.output = BlockOutput::Success(String::new());
                            }
                            block.exit_code = Some(0);
                        }
                        self.app.set_ai_thinking(false);
                    }
                    AiUpdate::Error { block_id, message } => {
                        self.app.fail_block(&block_id, message, String::new(), 1);
                        self.app.set_ai_thinking(false);
                    }
                }
            }

            // Tick animations
            self.app.tick();
        }

        Ok(())
    }

    /// Handle a key event, returns true if should exit
    async fn handle_key_event(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        cmd_tx: &mpsc::UnboundedSender<CommandUpdate>,
        ai_tx: &mpsc::UnboundedSender<AiUpdate>,
    ) -> Result<bool> {
        match code {
            // Ctrl+C - cancel or clear
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                if self.app.input.is_empty() {
                    // Exit if input is empty
                    return Ok(true);
                } else {
                    // Clear input
                    self.app.input_clear();
                }
            }

            // Ctrl+D - exit
            KeyCode::Char('d') if modifiers.contains(KeyModifiers::CONTROL) => {
                if self.app.input.is_empty() {
                    return Ok(true);
                }
            }

            // Ctrl+L - clear screen (keep history)
            KeyCode::Char('l') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.app.blocks.clear();
                self.app.mark_dirty();
            }

            // Ctrl+A - move to start
            KeyCode::Char('a') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.app.cursor_home();
            }

            // Ctrl+E - move to end
            KeyCode::Char('e') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.app.cursor_end();
            }

            // Ctrl+U - clear line
            KeyCode::Char('u') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.app.input_clear();
            }

            // Regular character input
            KeyCode::Char(c) => {
                self.app.input_push(c);
            }

            // Backspace
            KeyCode::Backspace => {
                self.app.input_pop();
            }

            // Delete
            KeyCode::Delete => {
                self.app.input_delete();
            }

            // Arrow keys
            KeyCode::Left => {
                self.app.cursor_left();
            }
            KeyCode::Right => {
                self.app.cursor_right();
            }
            KeyCode::Up => {
                self.app.history_up();
            }
            KeyCode::Down => {
                self.app.history_down();
            }

            // Home/End
            KeyCode::Home => {
                self.app.cursor_home();
            }
            KeyCode::End => {
                self.app.cursor_end();
            }

            // Page Up/Down for scrolling
            KeyCode::PageUp => {
                self.app.scroll_page_up();
            }
            KeyCode::PageDown => {
                self.app.scroll_page_down();
            }

            // Enter - submit command
            KeyCode::Enter => {
                let input = self.app.input_submit();
                if !input.is_empty() {
                    self.execute_input(&input, cmd_tx.clone(), ai_tx.clone())
                        .await?;
                }
            }

            // Escape - cancel/clear
            KeyCode::Esc => {
                self.app.input_clear();
            }

            _ => {}
        }

        Ok(false)
    }

    /// Execute user input
    async fn execute_input(
        &mut self,
        input: &str,
        cmd_tx: mpsc::UnboundedSender<CommandUpdate>,
        ai_tx: mpsc::UnboundedSender<AiUpdate>,
    ) -> Result<()> {
        let input = input.trim();

        // Check for AI command
        if let Some(ai_cmd) = ShellTuiApp::parse_ai_command(input) {
            return self.execute_ai_command(ai_cmd, input, ai_tx).await;
        }

        // Check for built-in commands
        if ShellTuiApp::is_builtin_command(input) {
            return self.execute_builtin(input);
        }

        // Execute as shell command
        self.execute_shell_command(input, cmd_tx).await
    }

    /// Execute a built-in command
    fn execute_builtin(&mut self, input: &str) -> Result<()> {
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let cmd = parts[0].to_lowercase();
        let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

        let prompt = self.app.current_prompt();

        match cmd.as_str() {
            "exit" | "quit" => {
                // Signal exit by adding a special block
                let block = CommandBlock::system("Goodbye!".to_string(), prompt);
                self.app.add_block(block);
                // The event loop will check for exit condition
                return Ok(());
            }

            "cd" => {
                let mut block =
                    CommandBlock::new(input.to_string(), BlockType::ShellCommand, prompt);
                match self.app.change_directory(args) {
                    Ok(()) => {
                        block.complete(String::new(), 0);
                    }
                    Err(e) => {
                        block.fail(e, String::new(), 1);
                    }
                }
                self.app.add_block(block);
            }

            "pwd" => {
                let mut block =
                    CommandBlock::new(input.to_string(), BlockType::ShellCommand, prompt);
                block.complete(self.app.cwd.display().to_string(), 0);
                self.app.add_block(block);
            }

            "clear" => {
                self.app.blocks.clear();
                self.app.mark_dirty();
            }

            "history" => {
                let mut block =
                    CommandBlock::new(input.to_string(), BlockType::ShellCommand, prompt);
                let history_text: String = self
                    .app
                    .command_history
                    .iter()
                    .enumerate()
                    .map(|(i, cmd)| format!("{:5}  {}", i + 1, cmd))
                    .collect::<Vec<_>>()
                    .join("\n");
                block.complete(history_text, 0);
                self.app.add_block(block);
            }

            "export" => {
                let mut block =
                    CommandBlock::new(input.to_string(), BlockType::ShellCommand, prompt);
                if args.is_empty() {
                    // Show exports
                    let exports: String = self
                        .app
                        .env_vars
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<_>>()
                        .join("\n");
                    block.complete(exports, 0);
                } else if let Some(pos) = args.find('=') {
                    let key = args[..pos].trim().to_string();
                    let value = args[pos + 1..]
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string();
                    self.app.env_vars.insert(key.clone(), value.clone());
                    std::env::set_var(&key, &value);
                    block.complete(String::new(), 0);
                } else {
                    block.fail("Usage: export KEY=VALUE".to_string(), String::new(), 1);
                }
                self.app.add_block(block);
            }

            "env" => {
                let mut block =
                    CommandBlock::new(input.to_string(), BlockType::ShellCommand, prompt);
                let env_text: String = std::env::vars()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join("\n");
                block.complete(env_text, 0);
                self.app.add_block(block);
            }

            "help" => {
                let mut block =
                    CommandBlock::new(input.to_string(), BlockType::ShellCommand, prompt);
                let help_text = r#"Safe Coder Shell Commands:

Shell Commands:
  cd <path>         Change directory
  pwd               Print working directory
  history           Show command history
  clear             Clear screen
  export KEY=VAL    Set environment variable
  env               Show all environment variables
  exit, quit        Exit shell

AI Commands (prefix with @):
  @connect          Connect to AI
  @disconnect       Disconnect from AI
  @ <query>         Ask AI for help (with shell context)
  @orchestrate      Run multi-agent task

Keyboard Shortcuts:
  Ctrl+C            Cancel/clear input (or exit if empty)
  Ctrl+L            Clear screen
  Ctrl+A/E          Move to start/end of line
  Ctrl+U            Clear input line
  Up/Down           Navigate command history
  PageUp/PageDown   Scroll output"#;
                block.complete(help_text.to_string(), 0);
                self.app.add_block(block);
            }

            _ => {
                let mut block =
                    CommandBlock::new(input.to_string(), BlockType::ShellCommand, prompt);
                block.fail(
                    format!("Unknown built-in command: {}", cmd),
                    String::new(),
                    127,
                );
                self.app.add_block(block);
            }
        }

        Ok(())
    }

    /// Execute a shell command asynchronously
    async fn execute_shell_command(
        &mut self,
        input: &str,
        tx: mpsc::UnboundedSender<CommandUpdate>,
    ) -> Result<()> {
        let prompt = self.app.current_prompt();
        let block = CommandBlock::new(input.to_string(), BlockType::ShellCommand, prompt);
        let block_id = block.id.clone();
        self.app.add_block(block);

        // Spawn async command execution
        let input_owned = input.to_string();
        let cwd = self.app.cwd.clone();
        let env_vars = self.app.env_vars.clone();

        tokio::spawn(async move {
            let result =
                execute_command_async(input_owned, cwd, env_vars, block_id.clone(), tx.clone())
                    .await;
            if let Err(e) = result {
                let _ = tx.send(CommandUpdate::Failed {
                    block_id,
                    message: e.to_string(),
                    stderr: String::new(),
                    exit_code: 1,
                });
            }
        });

        Ok(())
    }

    /// Execute an AI command
    async fn execute_ai_command(
        &mut self,
        cmd: AiCommand,
        original_input: &str,
        tx: mpsc::UnboundedSender<AiUpdate>,
    ) -> Result<()> {
        match cmd {
            AiCommand::Connect => {
                self.connect_ai().await?;
            }

            AiCommand::Disconnect => {
                self.disconnect_ai();
            }

            AiCommand::Query(query) => {
                if !self.app.ai_connected {
                    let prompt = self.app.current_prompt();
                    let mut block = CommandBlock::system(
                        "AI not connected. Run @connect first.".to_string(),
                        prompt,
                    );
                    self.app.add_block(block);
                    return Ok(());
                }

                // Create AI query block
                let prompt = self.app.current_prompt();
                let block = CommandBlock::new(query.clone(), BlockType::AiQuery, prompt);
                let block_id = block.id.clone();
                self.app.add_block(block);
                self.app.set_ai_thinking(true);

                // Build context and send to AI
                let context = self.app.build_ai_context();
                let full_query = format!(
                    "Context from shell session:\n{}\n\nUser query: {}",
                    context, query
                );

                // Clone session for async task
                if let Some(session) = &self.app.session {
                    let session = Arc::clone(session);
                    let tx = tx.clone();

                    tokio::spawn(async move {
                        let mut session = session.lock().await;
                        match session.send_message(full_query).await {
                            Ok(response) => {
                                let _ = tx.send(AiUpdate::Response {
                                    block_id: block_id.clone(),
                                    text: response,
                                });
                                let _ = tx.send(AiUpdate::Complete { block_id });
                            }
                            Err(e) => {
                                let _ = tx.send(AiUpdate::Error {
                                    block_id,
                                    message: e.to_string(),
                                });
                            }
                        }
                    });
                }
            }

            AiCommand::Orchestrate(task) => {
                if task.is_empty() {
                    let prompt = self.app.current_prompt();
                    let mut block = CommandBlock::system(
                        "Usage: @orchestrate <task description>".to_string(),
                        prompt,
                    );
                    self.app.add_block(block);
                    return Ok(());
                }

                // Create orchestration block
                let prompt = self.app.current_prompt();
                let block = CommandBlock::new(task.clone(), BlockType::Orchestration, prompt);
                let block_id = block.id.clone();
                self.app.add_block(block);

                // TODO: Implement orchestration integration
                // For now, show a placeholder
                let prompt = self.app.current_prompt();
                let mut msg_block = CommandBlock::system(
                    "Orchestration support coming soon. Use the 'safe-coder orchestrate' command for now.".to_string(),
                    prompt,
                );
                self.app.add_block(msg_block);

                if let Some(block) = self.app.get_block_mut(&block_id) {
                    block.complete("See above".to_string(), 0);
                }
            }
        }

        Ok(())
    }

    /// Connect to AI service
    async fn connect_ai(&mut self) -> Result<()> {
        if self.app.ai_connected {
            let prompt = self.app.current_prompt();
            let block = CommandBlock::system("Already connected to AI.".to_string(), prompt);
            self.app.add_block(block);
            return Ok(());
        }

        let prompt = self.app.current_prompt();
        let mut block = CommandBlock::new("connect".to_string(), BlockType::AiQuery, prompt);

        match Session::new(self.config.clone(), self.app.cwd.clone()).await {
            Ok(session) => {
                self.app.session = Some(Arc::new(Mutex::new(session)));
                self.app.set_ai_connected(true);
                block.complete(
                    "Connected to AI. Use @ <query> to ask questions.".to_string(),
                    0,
                );
            }
            Err(e) => {
                block.fail(
                    format!("Failed to connect: {}", e),
                    "Make sure you have configured an API key or run 'safe-coder login'"
                        .to_string(),
                    1,
                );
            }
        }

        self.app.add_block(block);
        Ok(())
    }

    /// Disconnect from AI service
    fn disconnect_ai(&mut self) {
        let prompt = self.app.current_prompt();
        let mut block = CommandBlock::new("disconnect".to_string(), BlockType::AiQuery, prompt);

        if self.app.ai_connected {
            self.app.session = None;
            self.app.set_ai_connected(false);
            block.complete("Disconnected from AI.".to_string(), 0);
        } else {
            block.complete("Not connected to AI.".to_string(), 0);
        }

        self.app.add_block(block);
    }
}

/// Execute a command asynchronously and stream output
async fn execute_command_async(
    command: String,
    cwd: PathBuf,
    env_vars: std::collections::HashMap<String, String>,
    block_id: String,
    tx: mpsc::UnboundedSender<CommandUpdate>,
) -> Result<()> {
    let mut child = TokioCommand::new("sh")
        .arg("-c")
        .arg(&command)
        .current_dir(&cwd)
        .envs(&env_vars)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to spawn command")?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Spawn tasks to read stdout and stderr
    let block_id_stdout = block_id.clone();
    let tx_stdout = tx.clone();
    let stdout_task = tokio::spawn(async move {
        if let Some(stdout) = stdout {
            let reader = TokioBufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_stdout.send(CommandUpdate::Output {
                    block_id: block_id_stdout.clone(),
                    line,
                });
            }
        }
    });

    let block_id_stderr = block_id.clone();
    let tx_stderr = tx.clone();
    let stderr_task = tokio::spawn(async move {
        if let Some(stderr) = stderr {
            let reader = TokioBufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = tx_stderr.send(CommandUpdate::Output {
                    block_id: block_id_stderr.clone(),
                    line: format!("stderr: {}", line),
                });
            }
        }
    });

    // Wait for command to complete
    let status = child.wait().await?;

    // Wait for output tasks
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    let exit_code = status.code().unwrap_or(1);
    tx.send(CommandUpdate::Complete {
        block_id,
        exit_code,
    })?;

    Ok(())
}

/// Run the shell TUI (convenience function)
pub async fn run_shell_tui(project_path: PathBuf, auto_connect_ai: bool) -> Result<()> {
    let config = Config::load()?;
    let mut runner = ShellTuiRunner::new(project_path, config);

    if auto_connect_ai {
        runner.run_with_ai().await
    } else {
        runner.run().await
    }
}
