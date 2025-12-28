//! Interactive shell mode for safe-coder
//!
//! This module provides a standalone shell mode that can be used to run
//! commands directly, with optional AI assistance when needed.

use anyhow::{Context, Result};
use std::collections::VecDeque;
use std::env;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

use crate::config::Config;
use crate::session::Session;

/// Maximum number of commands to keep in history
const MAX_HISTORY_SIZE: usize = 1000;

/// Shell mode for interactive command execution
pub struct Shell {
    /// Current working directory
    cwd: PathBuf,
    /// Command history
    history: VecDeque<String>,
    /// History position for navigation
    history_pos: usize,
    /// Optional coding session for AI assistance
    session: Option<Session>,
    /// Configuration
    config: Config,
    /// Last exit code
    last_exit_code: i32,
    /// Environment variables set in this shell
    env_vars: std::collections::HashMap<String, String>,
}

impl Shell {
    /// Create a new shell instance
    pub async fn new(path: PathBuf) -> Result<Self> {
        let config = Config::load().unwrap_or_default();
        let cwd = path.canonicalize().context("Failed to resolve path")?;

        Ok(Self {
            cwd,
            history: VecDeque::with_capacity(MAX_HISTORY_SIZE),
            history_pos: 0,
            session: None,
            config,
            last_exit_code: 0,
            env_vars: std::collections::HashMap::new(),
        })
    }

    /// Initialize AI session for coding assistance
    pub async fn init_ai_session(&mut self) -> Result<()> {
        let session = Session::new(self.config.clone(), self.cwd.clone()).await?;
        self.session = Some(session);
        Ok(())
    }

    /// Get the shell prompt
    fn get_prompt(&self) -> String {
        let dir_name = self
            .cwd
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| self.cwd.display().to_string());

        let git_branch = self.get_git_branch().unwrap_or_default();
        let branch_info = if git_branch.is_empty() {
            String::new()
        } else {
            format!(" ({})", git_branch)
        };

        let status_indicator = if self.last_exit_code == 0 {
            "\x1b[32m❯\x1b[0m" // Green
        } else {
            "\x1b[31m❯\x1b[0m" // Red
        };

        let ai_indicator = if self.session.is_some() {
            "\x1b[35m🤖\x1b[0m " // Purple robot
        } else {
            ""
        };

        format!(
            "{}\x1b[36m{}\x1b[0m\x1b[33m{}\x1b[0m {} ",
            ai_indicator, dir_name, branch_info, status_indicator
        )
    }

    /// Get current git branch
    fn get_git_branch(&self) -> Option<String> {
        let output = std::process::Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(&self.cwd)
            .output()
            .ok()?;

        if output.status.success() {
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !branch.is_empty() {
                return Some(branch);
            }
        }
        None
    }

    /// Run the interactive shell loop
    pub async fn run(&mut self) -> Result<()> {
        self.print_welcome();

        loop {
            // Print prompt
            print!("{}", self.get_prompt());
            io::stdout().flush()?;

            // Read input
            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                break;
            }

            let input = input.trim();

            // Handle empty input
            if input.is_empty() {
                continue;
            }

            // Add to history
            self.add_to_history(input);

            // Parse and execute
            match self.execute_input(input).await {
                Ok(should_exit) => {
                    if should_exit {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("\x1b[31mError:\x1b[0m {}", e);
                    self.last_exit_code = 1;
                }
            }
        }

        println!("\nGoodbye!");
        Ok(())
    }

    /// Print welcome message
    fn print_welcome(&self) {
        println!(
            "\x1b[1;36m┌─────────────────────────────────────────────────────────────┐\x1b[0m"
        );
        println!("\x1b[1;36m│\x1b[0m  \x1b[1;35mSafe Coder Shell\x1b[0m - Interactive shell with AI assistance   \x1b[1;36m│\x1b[0m");
        println!(
            "\x1b[1;36m└─────────────────────────────────────────────────────────────┘\x1b[0m"
        );
        println!();
        println!("  \x1b[33mShell Commands:\x1b[0m");
        println!("    \x1b[32mcd <path>\x1b[0m        - Change directory (supports ~, relative, absolute)");
        println!("    \x1b[32mpwd\x1b[0m              - Print current working directory");
        println!("    \x1b[32mhistory\x1b[0m          - Show command history");
        println!("    \x1b[32mclear\x1b[0m            - Clear the screen");
        println!("    \x1b[32mexport KEY=VAL\x1b[0m   - Set environment variable");
        println!("    \x1b[32menv\x1b[0m              - Show all environment variables");
        println!("    \x1b[32mexit\x1b[0m, \x1b[32mquit\x1b[0m       - Exit the shell");
        println!();
        println!("  \x1b[33mAI Commands:\x1b[0m");
        println!("    \x1b[32mai-connect\x1b[0m       - Connect to AI for coding assistance");
        println!("    \x1b[32mai-disconnect\x1b[0m    - Disconnect from AI session");
        println!(
            "    \x1b[32mai <question>\x1b[0m    - Ask AI for help (requires ai-connect first)"
        );
        println!("    \x1b[32mchat\x1b[0m             - Enter interactive coding mode with tool execution");
        println!();
        println!("  \x1b[33mChat Mode (after running 'chat'):\x1b[0m");
        println!("    \x1b[32m!<command>\x1b[0m       - Run shell command without leaving chat");
        println!("    \x1b[32mexit\x1b[0m, \x1b[32mshell\x1b[0m      - Return to shell mode");
        println!();
        println!("  \x1b[33mOther:\x1b[0m");
        println!("    \x1b[32mhelp\x1b[0m, \x1b[32m?\x1b[0m          - Show this help message");
        println!("    \x1b[32m<any command>\x1b[0m    - Run as shell command (ls, git, etc.)");
        println!();
    }

    /// Add command to history
    fn add_to_history(&mut self, cmd: &str) {
        // Don't add empty or duplicate consecutive commands
        if cmd.is_empty() {
            return;
        }
        if let Some(last) = self.history.back() {
            if last == cmd {
                return;
            }
        }

        if self.history.len() >= MAX_HISTORY_SIZE {
            self.history.pop_front();
        }
        self.history.push_back(cmd.to_string());
        self.history_pos = self.history.len();
    }

    /// Execute input command
    async fn execute_input(&mut self, input: &str) -> Result<bool> {
        // Parse built-in commands
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let cmd = parts[0].to_lowercase();
        let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match cmd.as_str() {
            "exit" | "quit" => return Ok(true),

            "cd" => {
                self.change_directory(args)?;
            }

            "help" | "?" => {
                self.print_welcome();
            }

            "history" => {
                self.show_history();
            }

            "ai-connect" => {
                self.connect_ai().await?;
            }

            "ai-disconnect" => {
                self.disconnect_ai();
            }

            "ai" => {
                if args.is_empty() {
                    println!("Usage: ai <question or request>");
                    println!("Example: ai how do I list files recursively?");
                } else {
                    self.ask_ai(args).await?;
                }
            }

            "chat" => {
                self.enter_chat_mode().await?;
            }

            "clear" => {
                print!("\x1b[2J\x1b[1;1H");
            }

            "pwd" => {
                println!("{}", self.cwd.display());
            }

            "export" => {
                self.handle_export(args)?;
            }

            "env" => {
                self.show_env();
            }

            _ => {
                // Execute as shell command
                self.execute_shell_command(input).await?;
            }
        }

        Ok(false)
    }

    /// Change directory
    fn change_directory(&mut self, path: &str) -> Result<()> {
        let new_path = if path.is_empty() || path == "~" {
            dirs::home_dir().context("Could not find home directory")?
        } else if path.starts_with('~') {
            let home = dirs::home_dir().context("Could not find home directory")?;
            home.join(&path[2..])
        } else if path.starts_with('/') {
            PathBuf::from(path)
        } else {
            self.cwd.join(path)
        };

        let canonical = new_path
            .canonicalize()
            .with_context(|| format!("cd: {}: No such directory", path))?;

        if !canonical.is_dir() {
            anyhow::bail!("cd: {}: Not a directory", path);
        }

        self.cwd = canonical;
        Ok(())
    }

    /// Show command history
    fn show_history(&self) {
        for (i, cmd) in self.history.iter().enumerate() {
            println!("{:5}  {}", i + 1, cmd);
        }
    }

    /// Connect to AI session
    async fn connect_ai(&mut self) -> Result<()> {
        if self.session.is_some() {
            println!("\x1b[33mAlready connected to AI session.\x1b[0m");
            return Ok(());
        }

        println!("\x1b[36mConnecting to AI...\x1b[0m");
        match self.init_ai_session().await {
            Ok(()) => {
                // Note: We don't call session.start() here to avoid git auto-commits
                // in shell mode. The session is ready to use after init.
                println!("\x1b[32m✓ Connected to AI. Use 'ai <question>' for assistance.\x1b[0m");
            }
            Err(e) => {
                println!("\x1b[31m✗ Failed to connect: {}\x1b[0m", e);
                println!("  Make sure you have configured an API key or logged in.");
                println!("  Run: safe-coder login anthropic");
            }
        }
        Ok(())
    }

    /// Disconnect from AI session
    fn disconnect_ai(&mut self) {
        if self.session.is_some() {
            self.session = None;
            println!("\x1b[32m✓ Disconnected from AI session.\x1b[0m");
        } else {
            println!("\x1b[33mNot connected to AI.\x1b[0m");
        }
    }

    /// Ask AI for help
    async fn ask_ai(&mut self, question: &str) -> Result<()> {
        let session = match &mut self.session {
            Some(s) => s,
            None => {
                println!("\x1b[33mNot connected to AI. Run 'ai-connect' first.\x1b[0m");
                return Ok(());
            }
        };

        println!("\x1b[36m🤖 Thinking...\x1b[0m");

        match session.send_message(question.to_string()).await {
            Ok(response) => {
                println!("\n{}\n", response);
            }
            Err(e) => {
                println!("\x1b[31mAI Error: {}\x1b[0m", e);
            }
        }

        Ok(())
    }

    /// Enter full chat/coding mode
    async fn enter_chat_mode(&mut self) -> Result<()> {
        // Ensure AI is connected
        if self.session.is_none() {
            self.connect_ai().await?;
        }

        if self.session.is_none() {
            println!("\x1b[31mFailed to start chat mode - no AI connection.\x1b[0m");
            return Ok(());
        }

        println!("\n\x1b[1;35m━━━ Entering Chat Mode ━━━\x1b[0m");
        println!("Type your requests for AI coding assistance.");
        println!("Type 'exit' or 'shell' to return to shell mode.\n");

        loop {
            print!("\x1b[35mchat>\x1b[0m ");
            io::stdout().flush()?;

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                break;
            }

            let input = input.trim();

            if input.is_empty() {
                continue;
            }

            if input == "exit" || input == "shell" || input == "quit" {
                println!("\n\x1b[1;36m━━━ Returning to Shell Mode ━━━\x1b[0m\n");
                break;
            }

            // Check for shell passthrough - execute without borrowing session
            if input.starts_with('!') {
                let cmd = &input[1..];
                self.execute_shell_command(cmd).await?;
                continue;
            }

            // Send to AI - borrow session only for this operation
            println!("\x1b[36m🤖 Processing...\x1b[0m");
            if let Some(ref mut session) = self.session {
                match session.send_message(input.to_string()).await {
                    Ok(response) => {
                        println!("\n{}\n", response);
                    }
                    Err(e) => {
                        println!("\x1b[31mError: {}\x1b[0m", e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Execute a shell command with real-time output
    async fn execute_shell_command(&mut self, command: &str) -> Result<()> {
        // Show the command being executed
        println!("\x1b[33m❯ {}\x1b[0m", command);
        
        // Use tokio Command for async execution with piped output
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.cwd)
            .envs(&self.env_vars)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .stdin(std::process::Stdio::inherit()) // Keep stdin interactive
            .spawn()
            .context("Failed to execute command")?;
        
        // Get handles to stdout and stderr
        let stdout = child.stdout.take().context("Failed to capture stdout")?;
        let stderr = child.stderr.take().context("Failed to capture stderr")?;
        
        // Create async readers
        use tokio::io::{AsyncBufReadExt, BufReader};
        let mut stdout_reader = BufReader::new(stdout);
        let mut stderr_reader = BufReader::new(stderr);
        
        let mut stdout_line = String::new();
        let mut stderr_line = String::new();
        
        // Stream output in real-time using select! to handle both streams
        loop {
            tokio::select! {
                result = stdout_reader.read_line(&mut stdout_line) => {
                    match result {
                        Ok(0) => break, // EOF
                        Ok(_) => {
                            print!("{}", stdout_line);
                            io::stdout().flush()?;
                            stdout_line.clear();
                        }
                        Err(_) => break,
                    }
                }
                result = stderr_reader.read_line(&mut stderr_line) => {
                    match result {
                        Ok(0) => {}, // EOF on stderr, continue
                        Ok(_) => {
                            // Print stderr in red
                            print!("\x1b[31m{}\x1b[0m", stderr_line);
                            io::stdout().flush()?;
                            stderr_line.clear();
                        }
                        Err(_) => {},
                    }
                }
                _ = child.wait() => {
                    // Process has completed, read any remaining output
                    while let Ok(n) = stdout_reader.read_line(&mut stdout_line).await {
                        if n == 0 { break; }
                        print!("{}", stdout_line);
                        io::stdout().flush()?;
                        stdout_line.clear();
                    }
                    while let Ok(n) = stderr_reader.read_line(&mut stderr_line).await {
                        if n == 0 { break; }
                        print!("\x1b[31m{}\x1b[0m", stderr_line);
                        io::stdout().flush()?;
                        stderr_line.clear();
                    }
                    break;
                }
            }
        }
        
        // Get the final exit status
        let status = child.wait().await?;
        self.last_exit_code = status.code().unwrap_or(1);

        // Show exit status if command failed
        if !status.success() {
            println!("\x1b[31m[Exit status: {}]\x1b[0m", self.last_exit_code);
        }

        Ok(())
    }

    /// Handle export command
    fn handle_export(&mut self, args: &str) -> Result<()> {
        if args.is_empty() {
            // Show all exports
            for (k, v) in &self.env_vars {
                println!("{}={}", k, v);
            }
            return Ok(());
        }

        // Parse KEY=VALUE
        if let Some(pos) = args.find('=') {
            let key = args[..pos].trim().to_string();
            let value = args[pos + 1..].trim().to_string();

            // Remove quotes if present
            let value = value.trim_matches('"').trim_matches('\'').to_string();

            self.env_vars.insert(key.clone(), value.clone());
            env::set_var(&key, &value);
        } else {
            println!("Usage: export KEY=VALUE");
        }

        Ok(())
    }

    /// Show environment variables
    fn show_env(&self) {
        for (key, value) in env::vars() {
            println!("{}={}", key, value);
        }
    }
}

/// Run the shell mode
pub async fn run_shell(path: PathBuf, connect_ai: bool) -> Result<()> {
    let mut shell = Shell::new(path).await?;

    if connect_ai {
        shell.connect_ai().await?;
    }

    shell.run().await
}
