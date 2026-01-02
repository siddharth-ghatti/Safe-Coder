use anyhow::Result;
use crate::commands::CommandResult;
use crate::session::Session;

/// Slash command types
#[derive(Debug, Clone)]
pub enum SlashCommand {
    Help,
    Quit,
    Exit,
    Clear,
    Stats,
    Chat(ChatSubcommand),
    Memory(MemorySubcommand),
    Model(Option<String>),
    Restore(Option<String>),
    ApprovalMode(Option<String>),
    ExecutionMode(Option<String>),
    Summary,
    Compress,
    Settings,
    Tools,
    About,
    Copy,
    Directory(DirectorySubcommand),
    Init,
    Commands,
    Unknown(String),
}

#[derive(Debug, Clone)]
pub enum ChatSubcommand {
    Save(Option<String>),
    Resume(String),
    List,
    Delete(String),
    Share(String),
}

#[derive(Debug, Clone)]
pub enum MemorySubcommand {
    Add(String),
    Show,
    Refresh,
}

#[derive(Debug, Clone)]
pub enum DirectorySubcommand {
    Add(String),
    Show,
}

impl SlashCommand {
    /// Parse a slash command from input
    pub fn parse(input: &str) -> Self {
        let input = input.trim_start_matches('/').trim();
        let parts: Vec<&str> = input.split_whitespace().collect();

        if parts.is_empty() {
            return SlashCommand::Unknown(String::new());
        }

        let cmd = parts[0].to_lowercase();
        let args = &parts[1..];

        match cmd.as_str() {
            "help" | "?" => SlashCommand::Help,
            "quit" | "exit" => SlashCommand::Quit,
            "clear" => SlashCommand::Clear,
            "stats" => SlashCommand::Stats,
            "chat" => Self::parse_chat_subcommand(args),
            "memory" => Self::parse_memory_subcommand(args),
            "model" => SlashCommand::Model(args.get(0).map(|s| s.to_string())),
            "restore" => SlashCommand::Restore(args.get(0).map(|s| s.to_string())),
            "approval-mode" => SlashCommand::ApprovalMode(args.get(0).map(|s| s.to_string())),
            "mode" => SlashCommand::ExecutionMode(args.get(0).map(|s| s.to_string())),
            "summary" => SlashCommand::Summary,
            "compress" => SlashCommand::Compress,
            "settings" => SlashCommand::Settings,
            "tools" => SlashCommand::Tools,
            "about" => SlashCommand::About,
            "copy" => SlashCommand::Copy,
            "directory" | "dir" => Self::parse_directory_subcommand(args),
            "init" => SlashCommand::Init,
            "commands" => SlashCommand::Commands,
            _ => SlashCommand::Unknown(input.to_string()),
        }
    }

    fn parse_chat_subcommand(args: &[&str]) -> SlashCommand {
        if args.is_empty() {
            return SlashCommand::Chat(ChatSubcommand::List);
        }

        match args[0].to_lowercase().as_str() {
            "save" => SlashCommand::Chat(ChatSubcommand::Save(
                args.get(1).map(|s| s.to_string())
            )),
            "resume" => {
                if args.len() < 2 {
                    return SlashCommand::Unknown("chat resume requires a session ID".to_string());
                }
                SlashCommand::Chat(ChatSubcommand::Resume(args[1].to_string()))
            },
            "list" => SlashCommand::Chat(ChatSubcommand::List),
            "delete" => {
                if args.len() < 2 {
                    return SlashCommand::Unknown("chat delete requires a session ID".to_string());
                }
                SlashCommand::Chat(ChatSubcommand::Delete(args[1].to_string()))
            },
            "share" => {
                if args.len() < 2 {
                    return SlashCommand::Unknown("chat share requires a session ID".to_string());
                }
                SlashCommand::Chat(ChatSubcommand::Share(args[1].to_string()))
            },
            _ => SlashCommand::Unknown(format!("Unknown chat subcommand: {}", args[0])),
        }
    }

    fn parse_memory_subcommand(args: &[&str]) -> SlashCommand {
        if args.is_empty() {
            return SlashCommand::Memory(MemorySubcommand::Show);
        }

        match args[0].to_lowercase().as_str() {
            "add" => {
                let content = args[1..].join(" ");
                SlashCommand::Memory(MemorySubcommand::Add(content))
            },
            "show" => SlashCommand::Memory(MemorySubcommand::Show),
            "refresh" => SlashCommand::Memory(MemorySubcommand::Refresh),
            _ => SlashCommand::Unknown(format!("Unknown memory subcommand: {}", args[0])),
        }
    }

    fn parse_directory_subcommand(args: &[&str]) -> SlashCommand {
        if args.is_empty() {
            return SlashCommand::Directory(DirectorySubcommand::Show);
        }

        match args[0].to_lowercase().as_str() {
            "add" => {
                if args.len() < 2 {
                    return SlashCommand::Unknown("directory add requires a path".to_string());
                }
                SlashCommand::Directory(DirectorySubcommand::Add(args[1].to_string()))
            },
            "show" => SlashCommand::Directory(DirectorySubcommand::Show),
            _ => SlashCommand::Unknown(format!("Unknown directory subcommand: {}", args[0])),
        }
    }
}

/// Execute a slash command
pub async fn execute_slash_command(cmd: SlashCommand, session: &mut Session) -> Result<CommandResult> {
    match cmd {
        SlashCommand::Help => {
            let help_text = get_help_text();
            Ok(CommandResult::Message(help_text))
        },
        SlashCommand::Quit | SlashCommand::Exit => {
            Ok(CommandResult::Exit)
        },
        SlashCommand::Clear => {
            Ok(CommandResult::Clear)
        },
        SlashCommand::Stats => {
            let stats = session.get_stats().await?;
            Ok(CommandResult::Message(stats))
        },
        SlashCommand::Chat(subcmd) => {
            execute_chat_command(subcmd, session).await
        },
        SlashCommand::Memory(subcmd) => {
            execute_memory_command(subcmd, session).await
        },
        SlashCommand::Model(model) => {
            match model {
                Some(m) => {
                    session.switch_model(&m).await?;
                    Ok(CommandResult::Message(format!("✓ Switched to model: {}", m)))
                },
                None => {
                    let current = session.get_current_model();
                    Ok(CommandResult::Message(format!("Current model: {}", current)))
                }
            }
        },
        SlashCommand::Restore(file) => {
            session.restore_file(file.as_deref()).await?;
            Ok(CommandResult::Message("✓ File(s) restored from checkpoint".to_string()))
        },
        SlashCommand::ApprovalMode(mode) => {
            match mode {
                Some(m) => {
                    session.set_approval_mode(&m)?;
                    Ok(CommandResult::Message(format!("✓ Approval mode set to: {}", m)))
                },
                None => {
                    let current = session.get_approval_mode();
                    Ok(CommandResult::Message(format!("Current approval mode: {}", current)))
                }
            }
        },
        SlashCommand::ExecutionMode(mode) => {
            use crate::approval::ExecutionMode;
            match mode {
                Some(m) => {
                    let exec_mode = ExecutionMode::from_str(&m)?;
                    session.set_execution_mode(exec_mode);
                    let description = match exec_mode {
                        ExecutionMode::Plan => "Deep planning with user approval before execution",
                        ExecutionMode::Act => "Lightweight planning with auto-execution",
                    };
                    Ok(CommandResult::Message(format!("✓ Execution mode set to: {} ({})", m, description)))
                },
                None => {
                    let current = session.execution_mode();
                    let description = match current {
                        ExecutionMode::Plan => "Deep planning with user approval before execution",
                        ExecutionMode::Act => "Lightweight planning with auto-execution",
                    };
                    Ok(CommandResult::Message(format!("Current execution mode: {} ({})\n\nAvailable modes:\n  plan - Deep planning with user approval\n  act  - Auto-execution with brief summaries", current, description)))
                }
            }
        },
        SlashCommand::Summary => {
            let summary = session.generate_project_summary().await?;
            Ok(CommandResult::Message(summary))
        },
        SlashCommand::Compress => {
            session.compress_conversation().await?;
            Ok(CommandResult::Message("✓ Conversation compressed to save tokens".to_string()))
        },
        SlashCommand::Settings => {
            let settings = session.get_settings();
            Ok(CommandResult::Message(settings))
        },
        SlashCommand::Tools => {
            let tools = session.list_tools();
            Ok(CommandResult::Message(tools))
        },
        SlashCommand::About => {
            let about = get_about_text();
            Ok(CommandResult::Message(about))
        },
        SlashCommand::Copy => {
            session.copy_last_output()?;
            Ok(CommandResult::Message("✓ Copied last output to clipboard".to_string()))
        },
        SlashCommand::Directory(subcmd) => {
            execute_directory_command(subcmd, session).await
        },
        SlashCommand::Init => {
            session.init_project_context().await?;
            Ok(CommandResult::Message("✓ Created project context file".to_string()))
        },
        SlashCommand::Commands => {
            Ok(CommandResult::ShowCommandsModal)
        },
        SlashCommand::Unknown(cmd) => {
            Ok(CommandResult::Message(format!("Unknown command: /{}. Type /help for available commands.", cmd)))
        },
    }
}

async fn execute_chat_command(subcmd: ChatSubcommand, session: &mut Session) -> Result<CommandResult> {
    match subcmd {
        ChatSubcommand::Save(name) => {
            let id = session.save_chat(name).await?;
            Ok(CommandResult::Message(format!("✓ Chat saved with ID: {}", id)))
        },
        ChatSubcommand::Resume(id) => {
            session.resume_chat(&id).await?;
            Ok(CommandResult::Message(format!("✓ Resumed chat: {}", id)))
        },
        ChatSubcommand::List => {
            let chats = session.list_chats().await?;
            Ok(CommandResult::Message(chats))
        },
        ChatSubcommand::Delete(id) => {
            session.delete_chat(&id).await?;
            Ok(CommandResult::Message(format!("✓ Deleted chat: {}", id)))
        },
        ChatSubcommand::Share(id) => {
            let share_url = session.share_chat(&id).await?;
            Ok(CommandResult::Message(format!("Share URL: {}", share_url)))
        },
    }
}

async fn execute_memory_command(subcmd: MemorySubcommand, session: &mut Session) -> Result<CommandResult> {
    match subcmd {
        MemorySubcommand::Add(content) => {
            session.add_memory(&content).await?;
            Ok(CommandResult::Message("✓ Memory added".to_string()))
        },
        MemorySubcommand::Show => {
            let memory = session.show_memory().await?;
            Ok(CommandResult::Message(memory))
        },
        MemorySubcommand::Refresh => {
            session.refresh_memory().await?;
            Ok(CommandResult::Message("✓ Memory refreshed from SAFE_CODER.md".to_string()))
        },
    }
}

async fn execute_directory_command(subcmd: DirectorySubcommand, session: &mut Session) -> Result<CommandResult> {
    match subcmd {
        DirectorySubcommand::Add(path) => {
            session.add_directory(&path).await?;
            Ok(CommandResult::Message(format!("✓ Added directory to workspace: {}", path)))
        },
        DirectorySubcommand::Show => {
            let dirs = session.list_directories().await?;
            Ok(CommandResult::Message(dirs))
        },
    }
}

fn get_help_text() -> String {
    r#"Safe Coder - Available Commands

SLASH COMMANDS (/)
  /help, /?           Show this help message
  /commands           Show detailed commands reference
  /quit, /exit        Exit the session
  /clear              Clear the screen
  /stats              Show session statistics (tokens, time, etc.)

SESSION MANAGEMENT
  /chat save [name]   Save current conversation
  /chat resume <id>   Resume a saved conversation
  /chat list          List all saved conversations
  /chat delete <id>   Delete a saved conversation

MEMORY & CONTEXT
  /memory add <text>  Add instruction to memory
  /memory show        Show current memory/instructions
  /memory refresh     Reload from SAFE_CODER.md

CONFIGURATION
  /mode [plan|act]    Set execution mode (plan/act)
  /model [name]       Switch model or show current
  /approval-mode [mode]  Set approval mode (plan/default/auto-edit/yolo)
  /settings           Show current settings

EXECUTION MODES
  plan - Deep planning with detailed analysis and user approval
  act  - Lightweight planning with automatic execution (default)

PROJECT TOOLS
  /summary            Generate project summary
  /compress           Compress conversation to save tokens
  /restore [file]     Restore file(s) from checkpoint
  /tools              List available tools
  /dir add <path>     Add directory to workspace
  /dir show           Show workspace directories
  /init               Create project context file

OTHER
  /copy               Copy last output to clipboard
  /about              About Safe Coder

AT-COMMANDS (@)
  @file.rs            Attach file contents to your message
  @src/**/*.rs        Attach multiple files matching pattern

SHELL PASSTHROUGH (!)
  !ls -la             Execute shell command in sandbox

APPROVAL MODES
  plan      - Show execution plan before running
  default   - Ask before each tool use
  auto-edit - Auto-approve edits, ask for others
  yolo      - Auto-approve everything (use with caution)
"#.to_string()
}

fn get_about_text() -> String {
    format!(r#"Safe Coder v{}

An AI-powered coding assistant with git workspace isolation.
Built in Rust with security-first design.

Features:
  • Git workspace isolation for CLI sessions
  • Multi-LLM support (Claude, OpenAI, Ollama)
  • Git change tracking
  • Session management
  • Custom commands
  • Tool execution in isolated workspaces

Repository: https://github.com/siddharth-ghatti/safe-coder
License: MIT
"#, env!("CARGO_PKG_VERSION"))
}

pub fn get_commands_text() -> String {
    r#"📋 Available Commands Reference

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

🔧 SYSTEM COMMANDS
  /help, /?             Show main help message
  /commands             Show this commands reference (you are here!)
  /quit, /exit          Exit the application
  /clear                Clear the terminal screen
  /stats                Display session statistics and token usage
  /about                Show version and application information

💬 SESSION & CHAT MANAGEMENT
  /chat save [name]     Save current conversation with optional name
  /chat resume <id>     Resume a previously saved conversation
  /chat list            List all saved conversations
  /chat delete <id>     Delete a saved conversation
  /chat share <id>      Generate a shareable link for a conversation

🧠 MEMORY & CONTEXT
  /memory add <text>    Add custom instructions to AI memory
  /memory show          Display current memory and instructions
  /memory refresh       Reload instructions from SAFE_CODER.md

⚙️  CONFIGURATION & SETTINGS  
  /mode [plan|act]      Set execution mode:
                        • plan - Deep planning with user approval
                        • act  - Auto-execution with brief summaries
  /model [name]         Switch AI model or show current model
  /approval-mode [mode] Set approval mode:
                        • plan    - Show execution plan before running
                        • default - Ask before each tool use  
                        • auto-edit - Auto-approve edits, ask for others
                        • yolo    - Auto-approve everything (⚠️ use with caution)
  /settings             Show all current configuration settings

📁 PROJECT & WORKSPACE
  /summary              Generate a summary of the current project
  /compress             Compress conversation history to save tokens
  /restore [file]       Restore file(s) from git checkpoint
  /tools                List all available development tools
  /dir add <path>       Add directory to current workspace
  /dir show             Show all directories in workspace
  /init                 Create a project context file (SAFE_CODER.md)

📋 OTHER UTILITIES
  /copy                 Copy the last AI response to clipboard

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📎 FILE ATTACHMENT (@-commands)
  @file.rs              Attach a single file to your message
  @src/**/*.rs          Attach multiple files using glob patterns
  @README.md @src/      Attach multiple files and directories

🖥️  SHELL PASSTHROUGH (!-commands)
  !ls -la               Execute shell commands in a secure sandbox
  !git status           Run git commands safely within the project
  !cargo build          Execute build tools and development commands

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

💡 Tips:
  • Use Tab for autocompletion of commands and file paths
  • Use Ctrl+R for command history search
  • Most commands have short aliases (e.g., /? for /help)
  • Commands are case-insensitive
  • Use /help for a quick overview, /commands for this detailed reference

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"#.to_string()
}
