//! Shell-first TUI application state
//!
//! This module provides the state model for a Warp-like shell experience
//! where commands execute inline with visual blocks and AI is contextually available.

use chrono::{DateTime, Local};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::autocomplete::Autocomplete;
use super::file_picker::FilePicker;
use super::sidebar::SidebarState;
use super::spinner::Spinner;
use crate::config::Config;
use crate::planning::PlanEvent;
use crate::session::Session;
use crate::tools::AgentMode;

/// Permission mode for tool execution
/// Controls how much user approval is required
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    /// YOLO: Bypass ALL permissions - auto-approve everything
    Yolo,
    /// EDIT: Auto-approve file edits, ask for bash/other actions
    Edit,
    /// ASK: Ask permission for every action (safest)
    Ask,
}

impl PermissionMode {
    /// Cycle to next mode
    pub fn next(self) -> Self {
        match self {
            PermissionMode::Ask => PermissionMode::Edit,
            PermissionMode::Edit => PermissionMode::Yolo,
            PermissionMode::Yolo => PermissionMode::Ask,
        }
    }

    /// Get short display name
    pub fn short_name(&self) -> &'static str {
        match self {
            PermissionMode::Yolo => "YOLO",
            PermissionMode::Edit => "EDIT",
            PermissionMode::Ask => "ASK",
        }
    }

    /// Get description
    pub fn description(&self) -> &'static str {
        match self {
            PermissionMode::Yolo => "Auto-approve ALL actions",
            PermissionMode::Edit => "Auto-approve edits only",
            PermissionMode::Ask => "Ask for all actions",
        }
    }

    /// Get color hint (for UI)
    pub fn color_hint(&self) -> &'static str {
        match self {
            PermissionMode::Yolo => "red",
            PermissionMode::Edit => "amber",
            PermissionMode::Ask => "green",
        }
    }

    /// Check if a tool needs approval in this mode
    pub fn needs_approval(&self, tool_name: &str) -> bool {
        match self {
            PermissionMode::Yolo => false,
            PermissionMode::Edit => {
                // Auto-approve read, write, edit; ask for bash and others
                !matches!(tool_name, "read_file" | "write_file" | "edit_file")
            }
            PermissionMode::Ask => true,
        }
    }
}

impl Default for PermissionMode {
    fn default() -> Self {
        PermissionMode::Ask
    }
}

impl fmt::Display for PermissionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.short_name())
    }
}

/// Maximum number of commands to keep in history
const MAX_HISTORY_SIZE: usize = 1000;

/// Default number of recent commands to include in AI context
const DEFAULT_AI_CONTEXT_COMMANDS: usize = 10;

/// Command suggestion for autocomplete
#[derive(Debug, Clone)]
pub struct CommandSuggestion {
    /// The command itself (e.g., "/help", "/connect")
    pub command: String,
    /// Short description of what the command does
    pub description: String,
    /// Optional longer help text
    pub usage: Option<String>,
}

/// Command autocomplete state
#[derive(Debug, Clone)]
pub struct CommandAutocomplete {
    /// Current suggestions based on input
    pub suggestions: Vec<CommandSuggestion>,
    /// Selected suggestion index  
    pub selected: usize,
    /// Whether the autocomplete is visible
    pub visible: bool,
    /// The current input prefix being completed
    pub prefix: String,
}

impl CommandAutocomplete {
    pub fn new() -> Self {
        Self {
            suggestions: Vec::new(),
            selected: 0,
            visible: false,
            prefix: String::new(),
        }
    }

    /// Get all available commands with descriptions
    fn get_all_commands() -> Vec<CommandSuggestion> {
        vec![
            // System commands
            CommandSuggestion {
                command: "/help".to_string(),
                description: "Show help information".to_string(),
                usage: Some("Show available commands and their descriptions".to_string()),
            },
            CommandSuggestion {
                command: "/commands".to_string(),
                description: "Show detailed commands reference".to_string(),
                usage: Some("Open full commands modal with comprehensive help".to_string()),
            },
            CommandSuggestion {
                command: "/quit".to_string(),
                description: "Exit the application".to_string(),
                usage: Some("Exit safe-coder session".to_string()),
            },
            CommandSuggestion {
                command: "/exit".to_string(),
                description: "Exit the application".to_string(),
                usage: Some("Exit safe-coder session".to_string()),
            },
            CommandSuggestion {
                command: "/clear".to_string(),
                description: "Clear the screen".to_string(),
                usage: Some("Clear the terminal display".to_string()),
            },
            CommandSuggestion {
                command: "/stats".to_string(),
                description: "Show session statistics".to_string(),
                usage: Some("Display token usage, time, and other statistics".to_string()),
            },
            CommandSuggestion {
                command: "/about".to_string(),
                description: "About Safe Coder".to_string(),
                usage: Some("Show version and application information".to_string()),
            },
            
            // Chat and session management
            CommandSuggestion {
                command: "/chat".to_string(),
                description: "Chat session management".to_string(),
                usage: Some("/chat save [name] | resume <id> | list | delete <id> | share <id>".to_string()),
            },
            CommandSuggestion {
                command: "/sessions".to_string(),
                description: "List all saved sessions".to_string(),
                usage: Some("Alias for /chat list".to_string()),
            },
            
            // Undo/Redo
            CommandSuggestion {
                command: "/undo".to_string(),
                description: "Undo the last change".to_string(),
                usage: Some("Revert to previous git commit".to_string()),
            },
            CommandSuggestion {
                command: "/redo".to_string(),
                description: "Redo a previously undone change".to_string(),
                usage: Some("Re-apply previously undone changes".to_string()),
            },
            
            // Memory and context
            CommandSuggestion {
                command: "/memory".to_string(),
                description: "Memory management".to_string(),
                usage: Some("/memory add <text> | show | refresh".to_string()),
            },
            CommandSuggestion {
                command: "/compact".to_string(),
                description: "Compact context to save tokens".to_string(),
                usage: Some("Manually compress conversation history".to_string()),
            },
            
            // Configuration
            CommandSuggestion {
                command: "/mode".to_string(),
                description: "Set execution mode".to_string(),
                usage: Some("/mode [plan|act] - Toggle between planning and execution modes".to_string()),
            },
            CommandSuggestion {
                command: "/agent".to_string(),
                description: "Change agent mode".to_string(),
                usage: Some("Alias for /mode".to_string()),
            },
            CommandSuggestion {
                command: "/model".to_string(),
                description: "Switch AI model".to_string(),
                usage: Some("/model [name] - Switch model or show current".to_string()),
            },
            CommandSuggestion {
                command: "/models".to_string(),
                description: "List available models".to_string(),
                usage: Some("Show models available for current provider".to_string()),
            },
            CommandSuggestion {
                command: "/provider".to_string(),
                description: "Switch AI provider".to_string(),
                usage: Some("/provider [anthropic|copilot|openai|openrouter|ollama]".to_string()),
            },
            CommandSuggestion {
                command: "/login".to_string(),
                description: "Login to provider".to_string(),
                usage: Some("/login [copilot|anthropic] - Authenticate with provider".to_string()),
            },
            CommandSuggestion {
                command: "/approval-mode".to_string(),
                description: "Set approval mode".to_string(),
                usage: Some("/approval-mode [plan|default|auto-edit|yolo]".to_string()),
            },
            CommandSuggestion {
                command: "/settings".to_string(),
                description: "Show current settings".to_string(),
                usage: Some("Display all configuration settings".to_string()),
            },
            
            // Project tools
            CommandSuggestion {
                command: "/summary".to_string(),
                description: "Generate project summary".to_string(),
                usage: Some("Create a summary of the current project".to_string()),
            },
            CommandSuggestion {
                command: "/compress".to_string(),
                description: "Compress conversation".to_string(),
                usage: Some("Compress conversation to save tokens".to_string()),
            },
            CommandSuggestion {
                command: "/restore".to_string(),
                description: "Restore file(s) from git".to_string(),
                usage: Some("/restore [file] - Restore from git checkpoint".to_string()),
            },
            CommandSuggestion {
                command: "/tools".to_string(),
                description: "List available tools".to_string(),
                usage: Some("Show all development tools available to AI".to_string()),
            },
            CommandSuggestion {
                command: "/directory".to_string(),
                description: "Workspace directory management".to_string(),
                usage: Some("/directory add <path> | show".to_string()),
            },
            CommandSuggestion {
                command: "/dir".to_string(),
                description: "Workspace directory management".to_string(),
                usage: Some("Alias for /directory".to_string()),
            },
            CommandSuggestion {
                command: "/init".to_string(),
                description: "Initialize project context".to_string(),
                usage: Some("Create SAFE_CODER.md project context file".to_string()),
            },
            
            // Checkpoints
            CommandSuggestion {
                command: "/checkpoint".to_string(),
                description: "Git-agnostic snapshots".to_string(),
                usage: Some("/checkpoint list | restore <id> | restore latest | delete <id>".to_string()),
            },
            CommandSuggestion {
                command: "/cp".to_string(),
                description: "Git-agnostic snapshots".to_string(),
                usage: Some("Alias for /checkpoint".to_string()),
            },
            
            // Skills
            CommandSuggestion {
                command: "/skill".to_string(),
                description: "Skill management".to_string(),
                usage: Some("/skill list | activate <name> | deactivate <name> | info <name>".to_string()),
            },
            CommandSuggestion {
                command: "/skills".to_string(),
                description: "Skill management".to_string(),
                usage: Some("Alias for /skill".to_string()),
            },
            
            // Unified planning
            CommandSuggestion {
                command: "/plan".to_string(),
                description: "Show planning status".to_string(),
                usage: Some("/plan show | groups | history".to_string()),
            },
            
            // Other utilities
            CommandSuggestion {
                command: "/copy".to_string(),
                description: "Copy last output to clipboard".to_string(),
                usage: Some("Copy the last AI response to clipboard".to_string()),
            },
            
            // AI connection commands (specific to shell mode)
            CommandSuggestion {
                command: "/connect".to_string(),
                description: "Connect to AI service".to_string(),
                usage: Some("Establish connection to the AI assistant".to_string()),
            },
            CommandSuggestion {
                command: "/disconnect".to_string(),
                description: "Disconnect from AI service".to_string(),
                usage: Some("Close connection to the AI assistant".to_string()),
            },
            CommandSuggestion {
                command: "/orchestrate".to_string(),
                description: "Run complex tasks with orchestration".to_string(),
                usage: Some("/orchestrate <task description> - Execute multi-step tasks with parallel workers".to_string()),
            },
        ]
    }

    /// Update suggestions based on current input
    pub fn update(&mut self, input: &str) {
        if !input.starts_with('/') {
            self.visible = false;
            return;
        }

        self.prefix = input.to_string();
        self.suggestions.clear();
        self.selected = 0;

        // Split input into command and args
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let command_part = parts[0];
        let args_part = parts.get(1).map_or("", |s| *s);

        if parts.len() == 1 {
            // Still completing the main command (no space typed yet)
            let query = command_part[1..].to_lowercase(); // Remove the leading /

            // Filter commands that start with the query
            for cmd in Self::get_all_commands() {
                if cmd.command[1..].to_lowercase().starts_with(&query) {
                    self.suggestions.push(cmd);
                }
            }
        } else {
            // User has typed a space - completing arguments/subcommands
            // Only show suggestions for commands that have subcommands
            self.complete_arguments(command_part, args_part);
        }

        self.visible = !self.suggestions.is_empty();
    }
    
    /// Complete arguments for specific commands
    fn complete_arguments(&mut self, command: &str, args: &str) {
        match command {
            "/chat" => {
                let subcommands = vec![
                    CommandSuggestion {
                        command: "save".to_string(),
                        description: "Save current conversation".to_string(),
                        usage: Some("save [name] - Save with optional name".to_string()),
                    },
                    CommandSuggestion {
                        command: "resume".to_string(),
                        description: "Resume a saved conversation".to_string(),
                        usage: Some("resume <id> - Resume by ID".to_string()),
                    },
                    CommandSuggestion {
                        command: "list".to_string(),
                        description: "List all saved conversations".to_string(),
                        usage: Some("list - Show all saved chats".to_string()),
                    },
                    CommandSuggestion {
                        command: "delete".to_string(),
                        description: "Delete a saved conversation".to_string(),
                        usage: Some("delete <id> - Delete by ID".to_string()),
                    },
                    CommandSuggestion {
                        command: "share".to_string(),
                        description: "Generate shareable link".to_string(),
                        usage: Some("share <id> - Create share link".to_string()),
                    },
                ];
                self.filter_subcommands(subcommands, args);
            }
            "/memory" => {
                let subcommands = vec![
                    CommandSuggestion {
                        command: "add".to_string(),
                        description: "Add to memory".to_string(),
                        usage: Some("add <text> - Add custom instruction".to_string()),
                    },
                    CommandSuggestion {
                        command: "show".to_string(),
                        description: "Show current memory".to_string(),
                        usage: Some("show - Display all memory".to_string()),
                    },
                    CommandSuggestion {
                        command: "refresh".to_string(),
                        description: "Reload from SAFE_CODER.md".to_string(),
                        usage: Some("refresh - Reload instructions".to_string()),
                    },
                ];
                self.filter_subcommands(subcommands, args);
            }
            "/directory" | "/dir" => {
                let subcommands = vec![
                    CommandSuggestion {
                        command: "add".to_string(),
                        description: "Add directory to workspace".to_string(),
                        usage: Some("add <path> - Add directory".to_string()),
                    },
                    CommandSuggestion {
                        command: "show".to_string(),
                        description: "Show workspace directories".to_string(),
                        usage: Some("show - List directories".to_string()),
                    },
                ];
                self.filter_subcommands(subcommands, args);
            }
            "/checkpoint" | "/cp" => {
                let subcommands = vec![
                    CommandSuggestion {
                        command: "list".to_string(),
                        description: "List all checkpoints".to_string(),
                        usage: Some("list - Show all saved checkpoints".to_string()),
                    },
                    CommandSuggestion {
                        command: "restore".to_string(),
                        description: "Restore a checkpoint".to_string(),
                        usage: Some("restore <id|latest> - Restore checkpoint".to_string()),
                    },
                    CommandSuggestion {
                        command: "delete".to_string(),
                        description: "Delete a checkpoint".to_string(),
                        usage: Some("delete <id> - Remove checkpoint".to_string()),
                    },
                ];
                self.filter_subcommands(subcommands, args);
            }
            "/skill" | "/skills" => {
                let subcommands = vec![
                    CommandSuggestion {
                        command: "list".to_string(),
                        description: "List all skills".to_string(),
                        usage: Some("list - Show available skills".to_string()),
                    },
                    CommandSuggestion {
                        command: "activate".to_string(),
                        description: "Activate a skill".to_string(),
                        usage: Some("activate <name> - Enable skill".to_string()),
                    },
                    CommandSuggestion {
                        command: "deactivate".to_string(),
                        description: "Deactivate a skill".to_string(),
                        usage: Some("deactivate <name> - Disable skill".to_string()),
                    },
                    CommandSuggestion {
                        command: "info".to_string(),
                        description: "Show skill details".to_string(),
                        usage: Some("info <name> - Get skill information".to_string()),
                    },
                ];
                self.filter_subcommands(subcommands, args);
            }
            "/plan" => {
                let subcommands = vec![
                    CommandSuggestion {
                        command: "show".to_string(),
                        description: "Show current plan status".to_string(),
                        usage: Some("show - Display plan status".to_string()),
                    },
                    CommandSuggestion {
                        command: "groups".to_string(),
                        description: "Show step groups".to_string(),
                        usage: Some("groups - Show parallelism info".to_string()),
                    },
                    CommandSuggestion {
                        command: "history".to_string(),
                        description: "Show plan history".to_string(),
                        usage: Some("history - Show execution history".to_string()),
                    },
                ];
                self.filter_subcommands(subcommands, args);
            }
            "/mode" | "/agent" => {
                let modes = vec![
                    CommandSuggestion {
                        command: "plan".to_string(),
                        description: "Deep planning mode".to_string(),
                        usage: Some("plan - Deep planning with approval".to_string()),
                    },
                    CommandSuggestion {
                        command: "act".to_string(),
                        description: "Auto-execution mode".to_string(),
                        usage: Some("act - Lightweight auto-execution".to_string()),
                    },
                ];
                self.filter_subcommands(modes, args);
            }
            "/approval-mode" => {
                let modes = vec![
                    CommandSuggestion {
                        command: "plan".to_string(),
                        description: "Show execution plan before running".to_string(),
                        usage: Some("plan - Show plans before execution".to_string()),
                    },
                    CommandSuggestion {
                        command: "default".to_string(),
                        description: "Ask before each tool use".to_string(),
                        usage: Some("default - Ask for each action".to_string()),
                    },
                    CommandSuggestion {
                        command: "auto-edit".to_string(),
                        description: "Auto-approve edits only".to_string(),
                        usage: Some("auto-edit - Auto-approve file edits".to_string()),
                    },
                    CommandSuggestion {
                        command: "yolo".to_string(),
                        description: "Auto-approve everything".to_string(),
                        usage: Some("yolo - Auto-approve all actions".to_string()),
                    },
                ];
                self.filter_subcommands(modes, args);
            }
            "/provider" => {
                let providers = vec![
                    CommandSuggestion {
                        command: "anthropic".to_string(),
                        description: "Anthropic (Claude)".to_string(),
                        usage: Some("anthropic - Use Claude models".to_string()),
                    },
                    CommandSuggestion {
                        command: "copilot".to_string(),
                        description: "GitHub Copilot".to_string(),
                        usage: Some("copilot - Use GitHub Copilot".to_string()),
                    },
                    CommandSuggestion {
                        command: "openai".to_string(),
                        description: "OpenAI (GPT)".to_string(),
                        usage: Some("openai - Use GPT models".to_string()),
                    },
                    CommandSuggestion {
                        command: "openrouter".to_string(),
                        description: "OpenRouter".to_string(),
                        usage: Some("openrouter - Use OpenRouter".to_string()),
                    },
                    CommandSuggestion {
                        command: "ollama".to_string(),
                        description: "Ollama (local)".to_string(),
                        usage: Some("ollama - Use local Ollama models".to_string()),
                    },
                ];
                self.filter_subcommands(providers, args);
            }
            "/login" => {
                let providers = vec![
                    CommandSuggestion {
                        command: "copilot".to_string(),
                        description: "GitHub Copilot (device flow)".to_string(),
                        usage: Some("copilot - Login via GitHub device flow".to_string()),
                    },
                    CommandSuggestion {
                        command: "anthropic".to_string(),
                        description: "Anthropic (API key)".to_string(),
                        usage: Some("anthropic - Set up API key".to_string()),
                    },
                ];
                self.filter_subcommands(providers, args);
            }
            _ => {
                // No subcommand completion for this command
            }
        }
    }
    
    /// Filter and add subcommands that match the current input
    fn filter_subcommands(&mut self, subcommands: Vec<CommandSuggestion>, args: &str) {
        let query = args.to_lowercase();
        for subcmd in subcommands {
            if subcmd.command.to_lowercase().starts_with(&query) {
                self.suggestions.push(subcmd);
            }
        }
    }

    /// Select next suggestion
    pub fn next(&mut self) {
        if !self.suggestions.is_empty() {
            self.selected = (self.selected + 1) % self.suggestions.len();
        }
    }

    /// Select previous suggestion
    pub fn prev(&mut self) {
        if !self.suggestions.is_empty() {
            if self.selected == 0 {
                self.selected = self.suggestions.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
    }

    /// Get current selected suggestion
    pub fn current(&self) -> Option<&CommandSuggestion> {
        self.suggestions.get(self.selected)
    }

    /// Hide the autocomplete
    pub fn hide(&mut self) {
        self.visible = false;
        self.suggestions.clear();
        self.selected = 0;
    }

    /// Apply current suggestion
    /// Returns the full command string to replace input with
    pub fn apply_current(&self) -> Option<String> {
        let suggestion = self.current()?;

        // Check if we're completing arguments (prefix contains a space)
        if self.prefix.contains(' ') {
            // Extract the command part (everything before the last space where args start)
            let parts: Vec<&str> = self.prefix.splitn(2, ' ').collect();
            if parts.len() >= 1 {
                // Return command + space + suggestion
                return Some(format!("{} {}", parts[0], suggestion.command));
            }
        }

        // Completing the main command - just return the suggestion
        Some(suggestion.command.clone())
    }
}

impl Default for CommandAutocomplete {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Debug, Clone, PartialEq)]
pub enum BlockType {
    /// Regular shell command (ls, git, cargo, etc.)
    ShellCommand,
    /// AI query (prefixed with @)
    AiQuery,
    /// Tool executed by AI (nested in AI response)
    AiToolExecution { tool_name: String },
    /// AI reasoning/explanation text (shown inline between tools)
    AiReasoning,
    /// System message (welcome, errors, notifications)
    SystemMessage,
    /// Orchestration task
    Orchestration,
    /// Subagent execution
    Subagent { kind: String },
}

/// Output state of a command block
#[derive(Debug, Clone)]
pub enum BlockOutput {
    /// Command is still running
    Pending,
    /// Command completed successfully
    Success(String),
    /// Command failed with error
    Error { message: String, stderr: String },
    /// Real-time streaming output (for long-running commands)
    Streaming { lines: Vec<String>, complete: bool },
}

/// File diff information for edit operations
#[derive(Debug, Clone)]
pub struct FileDiff {
    /// Path to the file that was edited
    pub path: String,
    /// Content before the edit
    pub old_content: String,
    /// Content after the edit
    pub new_content: String,
}

impl BlockOutput {
    pub fn is_pending(&self) -> bool {
        matches!(self, BlockOutput::Pending)
    }

    pub fn is_complete(&self) -> bool {
        match self {
            BlockOutput::Success(_) | BlockOutput::Error { .. } => true,
            BlockOutput::Streaming { complete, .. } => *complete,
            BlockOutput::Pending => false,
        }
    }

    pub fn get_text(&self) -> String {
        match self {
            BlockOutput::Pending => String::new(),
            BlockOutput::Success(s) => s.clone(),
            BlockOutput::Error { message, stderr } => {
                if stderr.is_empty() {
                    message.clone()
                } else {
                    format!("{}\n{}", message, stderr)
                }
            }
            BlockOutput::Streaming { lines, .. } => lines.join("\n"),
        }
    }
}

/// Shell prompt state at the time a command was entered
#[derive(Debug, Clone)]
pub struct ShellPrompt {
    /// Full current working directory
    pub cwd: PathBuf,
    /// Short display name (just the directory name)
    pub cwd_short: String,
    /// Current git branch if in a git repo
    pub git_branch: Option<String>,
    /// Exit code of previous command
    pub last_exit_code: i32,
    /// Whether AI was connected
    pub ai_connected: bool,
}

impl ShellPrompt {
    pub fn new(
        cwd: PathBuf,
        git_branch: Option<String>,
        last_exit_code: i32,
        ai_connected: bool,
    ) -> Self {
        let cwd_short = cwd
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| cwd.display().to_string());

        Self {
            cwd,
            cwd_short,
            git_branch,
            last_exit_code,
            ai_connected,
        }
    }
}

/// A command block represents one shell interaction (like Warp's blocks)
#[derive(Debug, Clone)]
pub struct CommandBlock {
    /// Unique identifier
    pub id: String,
    /// The prompt state when command was entered
    pub prompt: ShellPrompt,
    /// What the user typed
    pub input: String,
    /// Type of block
    pub block_type: BlockType,
    /// Output/result of the command
    pub output: BlockOutput,
    /// Exit code for shell commands
    pub exit_code: Option<i32>,
    /// When the command was started
    pub timestamp: DateTime<Local>,
    /// How long the command took (ms)
    pub duration_ms: Option<u64>,
    /// Whether the block is collapsed in the UI
    pub collapsed: bool,
    /// Child blocks (for AI tool executions)
    pub children: Vec<CommandBlock>,
    /// File diff for edit operations (tool blocks only)
    pub diff: Option<FileDiff>,
    /// Version counter for render cache invalidation (increments on content change)
    pub render_version: u32,
}

impl CommandBlock {
    /// Create a new command block
    pub fn new(input: String, block_type: BlockType, prompt: ShellPrompt) -> Self {
        Self {
            id: Uuid::new_v4().to_string()[..8].to_string(),
            prompt,
            input,
            block_type,
            output: BlockOutput::Pending,
            exit_code: None,
            timestamp: Local::now(),
            duration_ms: None,
            collapsed: false,
            children: Vec::new(),
            diff: None,
            render_version: 0,
        }
    }

    /// Create a system message block
    pub fn system(message: String, prompt: ShellPrompt) -> Self {
        let mut block = Self::new(String::new(), BlockType::SystemMessage, prompt);
        block.output = BlockOutput::Success(message);
        block.render_version = 1;
        block
    }

    /// Bump render version (call after any content change)
    #[inline]
    fn bump_version(&mut self) {
        self.render_version = self.render_version.wrapping_add(1);
    }

    /// Mark the block as completed with success
    pub fn complete(&mut self, output: String, exit_code: i32) {
        let elapsed = Local::now()
            .signed_duration_since(self.timestamp)
            .num_milliseconds() as u64;
        self.output = BlockOutput::Success(output);
        self.exit_code = Some(exit_code);
        self.duration_ms = Some(elapsed);
        self.bump_version();
    }

    /// Mark the block as failed
    pub fn fail(&mut self, message: String, stderr: String, exit_code: i32) {
        let elapsed = Local::now()
            .signed_duration_since(self.timestamp)
            .num_milliseconds() as u64;
        self.output = BlockOutput::Error { message, stderr };
        self.exit_code = Some(exit_code);
        self.duration_ms = Some(elapsed);
        self.bump_version();
    }

    /// Append streaming output
    pub fn append_output(&mut self, line: String) {
        match &mut self.output {
            BlockOutput::Pending => {
                self.output = BlockOutput::Streaming {
                    lines: vec![line],
                    complete: false,
                };
            }
            BlockOutput::Streaming { lines, .. } => {
                lines.push(line);
            }
            _ => {}
        }
        self.bump_version();
    }

    /// Mark streaming as complete
    pub fn complete_streaming(&mut self, exit_code: i32) {
        if let BlockOutput::Streaming { lines, complete } = &mut self.output {
            *complete = true;
            self.exit_code = Some(exit_code);
            let elapsed = Local::now()
                .signed_duration_since(self.timestamp)
                .num_milliseconds() as u64;
            self.duration_ms = Some(elapsed);
        }
        self.bump_version();
    }

    /// Add a child block (for AI tool executions)
    pub fn add_child(&mut self, child: CommandBlock) {
        self.children.push(child);
        self.bump_version();
    }

    /// Check if this block is still running
    pub fn is_running(&self) -> bool {
        !self.output.is_complete()
    }

    /// Get display duration string
    pub fn duration_display(&self) -> Option<String> {
        self.duration_ms.map(|ms| {
            if ms < 1000 {
                format!("{}ms", ms)
            } else if ms < 60000 {
                format!("{:.2}s", ms as f64 / 1000.0)
            } else {
                let secs = ms / 1000;
                format!("{}m {}s", secs / 60, secs % 60)
            }
        })
    }
}

/// Input mode for the shell
#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    /// Normal shell input
    Normal,
    /// AI mode (after typing @)
    AiPrefix,
    /// Slash command mode (after typing /)
    SlashCommand,
    /// History search mode (Ctrl+R)
    Search,
    /// Block selection mode
    BlockSelect,
}

/// Focus area in the UI
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusArea {
    /// Input line at bottom
    Input,
    /// Block list (for scrolling/selection)
    Blocks,
    /// Expanded view of a single block
    BlockDetail,
}

/// Main application state for shell-first TUI
pub struct ShellTuiApp {
    // === Shell State ===
    /// Current working directory
    pub cwd: PathBuf,
    /// Environment variables set in this shell session
    pub env_vars: HashMap<String, String>,
    /// Exit code of last command
    pub last_exit_code: i32,
    /// Project root path
    pub project_path: PathBuf,

    // === Block History ===
    /// Command blocks (Warp-style history)
    pub blocks: Vec<CommandBlock>,
    /// Command history for up-arrow navigation
    pub command_history: VecDeque<String>,
    /// Current position in history navigation
    pub history_pos: usize,

    // === AI State ===
    /// AI session for coding assistance
    pub session: Option<Arc<Mutex<Session>>>,
    /// Whether AI is connected
    pub ai_connected: bool,
    /// Whether AI is currently processing
    pub ai_thinking: bool,
    /// Number of recent commands to include in AI context
    pub ai_context_commands: usize,
    /// Configuration
    pub config: Config,
    /// Permission mode for tool execution (YOLO/EDIT/ASK)
    pub permission_mode: PermissionMode,
    /// Agent mode for tool availability (PLAN/BUILD)
    pub agent_mode: AgentMode,

    // === UI State ===
    /// Current input text
    pub input: String,
    /// Cursor position in input
    pub cursor_pos: usize,
    /// Current input mode
    pub input_mode: InputMode,
    /// Scroll offset for block list (0 = bottom/most recent)
    pub scroll_offset: usize,
    /// Whether user is "pinned" to bottom (auto-scroll on new content)
    pub auto_scroll: bool,
    /// Currently selected block index (for block selection mode)
    pub selected_block: Option<usize>,
    /// Current focus area
    pub focus: FocusArea,
    /// Search query (for Ctrl+R search)
    pub search_query: String,
    /// Search results (indices into command_history)
    pub search_results: Vec<usize>,
    /// Current search result index
    pub search_result_pos: usize,
    /// Autocomplete state
    pub autocomplete: Autocomplete,
    /// File picker for @mentions
    pub file_picker: FilePicker,
    /// Command autocomplete for slash commands
    pub command_autocomplete: CommandAutocomplete,
    /// Commands modal visibility
    pub commands_modal_visible: bool,

    // === Animation/Render State ===
    /// Whether UI needs to be redrawn
    pub needs_redraw: bool,
    /// Animation frame counter
    pub animation_frame: usize,
    /// Spinner for AI thinking
    pub spinner: Spinner,
    /// Start time for session
    pub start_time: DateTime<Local>,

    // === LSP State ===
    /// Running LSP servers (language name -> command)
    pub lsp_servers: Vec<(String, String, bool)>, // (language, command, running)
    /// LSP initialization status message (shown in status bar)
    pub lsp_status_message: Option<String>,
    /// Whether LSP initialization is in progress
    pub lsp_initializing: bool,

    // === Sidebar State ===
    /// Sidebar with plan progress, token usage, and connections
    pub sidebar: SidebarState,

    // === Plan Approval State ===
    /// Whether plan approval popup is visible
    pub plan_approval_visible: bool,
    /// The plan awaiting approval (contains summary and steps)
    pub pending_approval_plan: Option<crate::planning::TaskPlan>,
    /// Sender to approve/reject the plan (using unbounded since sender needs Clone)
    pub plan_approval_tx: Option<tokio::sync::mpsc::UnboundedSender<bool>>,
    /// Current plan step being executed (0-indexed)
    pub current_plan_step: usize,
    /// Whether plan execution is in progress
    pub plan_executing: bool,

    // === Render Cache ===
    /// Cached render width (invalidate cache if width changes)
    pub cached_render_width: usize,
    /// Total cached line count (for fast scrollbar calculation)
    pub cached_total_lines: usize,

    // === Provider/Model Display ===
    /// Display name for current model (e.g., "claude-sonnet-4", "gpt-4o")
    pub model_display: String,
}

impl ShellTuiApp {
    /// Create a new shell TUI application
    pub fn new(project_path: PathBuf, config: Config) -> Self {
        let cwd = project_path.clone();
        let _cwd_short = cwd
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "~".to_string());

        // Get model display name before moving config
        let model_display = config.llm.model.clone();

        let mut app = Self {
            cwd: cwd.clone(),
            env_vars: HashMap::new(),
            last_exit_code: 0,
            project_path: project_path.clone(),

            blocks: Vec::new(),
            command_history: VecDeque::with_capacity(MAX_HISTORY_SIZE),
            history_pos: 0,

            session: None,
            ai_connected: false,
            ai_thinking: false,
            ai_context_commands: DEFAULT_AI_CONTEXT_COMMANDS,
            config,
            permission_mode: PermissionMode::default(),
            agent_mode: AgentMode::default(),

            input: String::new(),
            cursor_pos: 0,
            input_mode: InputMode::Normal,
            scroll_offset: 0,
            auto_scroll: true, // Start pinned to bottom
            selected_block: None,
            focus: FocusArea::Input,
            search_query: String::new(),
            search_results: Vec::new(),
            search_result_pos: 0,
            autocomplete: Autocomplete::new(),
            file_picker: FilePicker::new(),
            command_autocomplete: CommandAutocomplete::new(),
            commands_modal_visible: false,

            needs_redraw: true,
            animation_frame: 0,
            spinner: Spinner::new(),
            start_time: Local::now(),

            lsp_servers: Vec::new(),
            lsp_status_message: None,
            lsp_initializing: true,

            sidebar: SidebarState::new(),

            plan_approval_visible: false,
            pending_approval_plan: None,
            plan_approval_tx: None,
            current_plan_step: 0,
            plan_executing: false,

            cached_render_width: 0,
            cached_total_lines: 0,

            model_display,
        };

        // Add welcome message
        let welcome = format!(
            "Welcome to Safe Coder Shell!\n\n\
             Project: {}\n\n\
             Usage:\n\
             • Shell commands work normally (ls, git, cargo, etc.)\n\
             • Just type naturally to ask AI for help\n\
             • Use @file.rs to include file context\n\n\
             Commands:\n\
             • /connect      - Connect to AI\n\
             • /disconnect   - Disconnect from AI\n\
             • /help         - Show all commands\n\
             • /mode         - Toggle permission mode\n\
             • exit          - Exit shell\n\n\
             Press Ctrl+C to cancel, Ctrl+P to change mode.",
            project_path.display()
        );
        let prompt = app.current_prompt();
        app.blocks.push(CommandBlock::system(welcome, prompt));

        app
    }

    /// Get current shell prompt state
    pub fn current_prompt(&self) -> ShellPrompt {
        ShellPrompt::new(
            self.cwd.clone(),
            self.get_git_branch(),
            self.last_exit_code,
            self.ai_connected,
        )
    }

    /// Get current git branch
    pub fn get_git_branch(&self) -> Option<String> {
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

    /// Mark app as needing redraw
    pub fn mark_dirty(&mut self) {
        self.needs_redraw = true;
    }

    /// Clear dirty flag
    pub fn clear_dirty(&mut self) {
        self.needs_redraw = false;
    }

    /// Tick animation state - optimized to minimize redraws
    pub fn tick(&mut self) {
        self.animation_frame = self.animation_frame.wrapping_add(1);

        // Only redraw for spinner animation every 4 ticks (~64ms at 60fps)
        // This is fast enough for smooth spinners but reduces CPU load
        if self.animation_frame % 4 == 0 {
            if self.ai_thinking {
                self.spinner.tick();
                self.needs_redraw = true;
            }

            // Cursor blink - only check when we might redraw anyway
            // Blink every ~20 ticks = ~320ms
            let cursor_phase = (self.animation_frame / 4) % 20;
            if cursor_phase == 0 || cursor_phase == 10 {
                self.needs_redraw = true;
            }
        }

        // Running block check - only every 8 ticks (~128ms) to reduce overhead
        // This is still responsive enough for spinner updates
        if self.animation_frame % 8 == 0 {
            // Only check last few blocks (most likely to be running)
            let has_running = self.blocks.iter().rev().take(5).any(|b| b.is_running());
            if has_running {
                self.needs_redraw = true;
            }
        }
    }

    // === Input Handling ===

    /// Push a character to input
    pub fn input_push(&mut self, c: char) {
        self.input.insert(self.cursor_pos, c);
        self.cursor_pos += 1;
        self.needs_redraw = true;

        // Update input mode based on prefix
        self.update_input_mode();

        // Hide autocomplete when typing (will show again on Tab)
        self.autocomplete.hide();
    }

    /// Pop a character from input (backspace)
    pub fn input_pop(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.input.remove(self.cursor_pos);
            self.needs_redraw = true;
            self.update_input_mode();

            // Hide autocomplete when deleting
            self.autocomplete.hide();
        }
    }

    /// Delete character at cursor (delete key)
    pub fn input_delete(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.input.remove(self.cursor_pos);
            self.needs_redraw = true;
        }
    }

    /// Move cursor left
    pub fn cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.needs_redraw = true;
        }
    }

    /// Move cursor right
    pub fn cursor_right(&mut self) {
        if self.cursor_pos < self.input.len() {
            self.cursor_pos += 1;
            self.needs_redraw = true;
        }
    }

    /// Move cursor to start
    pub fn cursor_home(&mut self) {
        self.cursor_pos = 0;
        self.needs_redraw = true;
    }

    /// Move cursor to end
    pub fn cursor_end(&mut self) {
        self.cursor_pos = self.input.len();
        self.needs_redraw = true;
    }

    /// Clear input
    pub fn input_clear(&mut self) {
        self.input.clear();
        self.cursor_pos = 0;
        self.input_mode = InputMode::Normal;
        self.autocomplete.hide();
        self.needs_redraw = true;
    }

    /// Submit input and return it
    pub fn input_submit(&mut self) -> String {
        let input = self.input.clone();
        self.input.clear();
        self.cursor_pos = 0;
        self.input_mode = InputMode::Normal;
        self.autocomplete.hide();
        self.scroll_to_bottom();
        self.needs_redraw = true;

        // Add to command history (skip empty and duplicates)
        if !input.is_empty() {
            if self.command_history.back() != Some(&input) {
                if self.command_history.len() >= MAX_HISTORY_SIZE {
                    self.command_history.pop_front();
                }
                self.command_history.push_back(input.clone());
            }
            self.history_pos = self.command_history.len();
        }

        input
    }

    // === Autocomplete ===

    /// Trigger autocomplete for current input
    pub fn trigger_autocomplete(&mut self) {
        // Don't autocomplete AI commands
        if self.input.starts_with('@') {
            return;
        }

        // If in slash command mode, update command suggestions
        if self.input.starts_with('/') {
            self.command_autocomplete.update(&self.input);
            // If there's exactly one match, apply it immediately
            if self.command_autocomplete.suggestions.len() == 1 {
                self.apply_autocomplete();
            }
        } else {
            // Regular autocomplete for paths/commands
            self.autocomplete.complete(&self.input, &self.cwd);
            // If there's exactly one match, apply it immediately
            if self.autocomplete.single_match() {
                self.apply_autocomplete();
            }
        }

        self.needs_redraw = true;
    }

    /// Check if autocomplete is currently visible (either command or file)
    pub fn autocomplete_visible(&self) -> bool {
        self.autocomplete.visible || self.command_autocomplete.visible
    }

    /// Apply autocomplete suggestion (either command or file)
    pub fn apply_autocomplete(&mut self) {
        if self.command_autocomplete.visible {
            if let Some(command) = self.command_autocomplete.apply_current() {
                self.input = command + " "; // Add space after command
                self.cursor_pos = self.input.len();
                self.command_autocomplete.hide();
                self.update_input_mode();
                self.needs_redraw = true;
            }
        } else if self.autocomplete.visible {
            if let Some(completion) = self.autocomplete.apply(&self.input) {
                self.input = completion;
                self.cursor_pos = self.input.len();
                self.autocomplete.hide();
                self.needs_redraw = true;
            }
        }
    }

    /// Navigate autocomplete suggestions
    pub fn autocomplete_next(&mut self) {
        if self.command_autocomplete.visible {
            self.command_autocomplete.next();
            self.needs_redraw = true;
        } else if self.autocomplete.visible {
            self.autocomplete.next();
            self.needs_redraw = true;
        }
    }

    /// Navigate autocomplete suggestions backwards
    pub fn autocomplete_prev(&mut self) {
        if self.command_autocomplete.visible {
            self.command_autocomplete.prev();
            self.needs_redraw = true;
        } else if self.autocomplete.visible {
            self.autocomplete.prev();
            self.needs_redraw = true;
        }
    }

    /// Update input mode based on current input
    fn update_input_mode(&mut self) {
        if self.input.starts_with('@') {
            self.input_mode = InputMode::AiPrefix;
            self.command_autocomplete.hide(); // Hide command autocomplete when in @mode
        } else if self.input.starts_with('/') {
            self.input_mode = InputMode::SlashCommand;
            // Update command autocomplete suggestions
            self.command_autocomplete.update(&self.input);
        } else {
            self.input_mode = InputMode::Normal;
            self.command_autocomplete.hide(); // Hide when not in slash mode
        }
    }

    // === History Navigation ===

    /// Navigate up in history
    pub fn history_up(&mut self) {
        if self.history_pos > 0 {
            self.history_pos -= 1;
            if let Some(cmd) = self.command_history.get(self.history_pos) {
                self.input = cmd.clone();
                self.cursor_pos = self.input.len();
                self.update_input_mode();
                self.needs_redraw = true;
            }
        }
    }

    /// Navigate down in history
    pub fn history_down(&mut self) {
        if self.history_pos < self.command_history.len() {
            self.history_pos += 1;
            if self.history_pos == self.command_history.len() {
                self.input.clear();
                self.cursor_pos = 0;
            } else if let Some(cmd) = self.command_history.get(self.history_pos) {
                self.input = cmd.clone();
                self.cursor_pos = self.input.len();
            }
            self.update_input_mode();
            self.needs_redraw = true;
        }
    }

    // === Block Management ===

    /// Add a new command block
    pub fn add_block(&mut self, block: CommandBlock) {
        self.blocks.push(block);
        self.auto_scroll_to_bottom(); // Only scroll if user is at bottom
        self.needs_redraw = true;
    }

    /// Get a mutable reference to the last block
    pub fn last_block_mut(&mut self) -> Option<&mut CommandBlock> {
        self.blocks.last_mut()
    }

    /// Get block by ID
    pub fn get_block_mut(&mut self, id: &str) -> Option<&mut CommandBlock> {
        self.blocks.iter_mut().find(|b| b.id == id)
    }

    /// Complete a block by ID
    pub fn complete_block(&mut self, id: &str, output: String, exit_code: i32) {
        if let Some(block) = self.get_block_mut(id) {
            block.complete(output, exit_code);
            self.last_exit_code = exit_code;
            self.needs_redraw = true;
        }
    }

    /// Fail a block by ID
    pub fn fail_block(&mut self, id: &str, message: String, stderr: String, exit_code: i32) {
        if let Some(block) = self.get_block_mut(id) {
            block.fail(message, stderr, exit_code);
            self.last_exit_code = exit_code;
            self.needs_redraw = true;
        }
    }

    /// Append output to a block
    pub fn append_to_block(&mut self, id: &str, line: String) {
        if let Some(block) = self.get_block_mut(id) {
            block.append_output(line);
            self.auto_scroll_to_bottom(); // Keep scrolling if user is at bottom
            self.needs_redraw = true;
        }
    }

    // === Scrolling ===

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(3); // Scroll 3 lines for smoother feel
        self.auto_scroll = false; // User scrolled up, disable auto-scroll
        self.needs_redraw = true;
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(3); // Scroll 3 lines for smoother feel
                                                                   // Re-enable auto-scroll if user scrolls back to bottom
        if self.scroll_offset == 0 {
            self.auto_scroll = true;
        }
        self.needs_redraw = true;
    }

    pub fn scroll_page_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(20);
        self.auto_scroll = false; // User scrolled up, disable auto-scroll
        self.needs_redraw = true;
    }

    pub fn scroll_page_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(20);
        // Re-enable auto-scroll if user scrolls back to bottom
        if self.scroll_offset == 0 {
            self.auto_scroll = true;
        }
        self.needs_redraw = true;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.auto_scroll = true; // Re-enable auto-scroll
        self.needs_redraw = true;
    }

    /// Auto-scroll to bottom only if user hasn't manually scrolled up
    pub fn auto_scroll_to_bottom(&mut self) {
        if self.auto_scroll {
            self.scroll_offset = 0;
            self.needs_redraw = true;
        }
    }

    // === AI State ===

    /// Set AI thinking state
    pub fn set_ai_thinking(&mut self, thinking: bool) {
        self.ai_thinking = thinking;
        self.needs_redraw = true;
    }

    /// Set AI connected state
    pub fn set_ai_connected(&mut self, connected: bool) {
        self.ai_connected = connected;
        self.needs_redraw = true;
    }

    /// Cycle to next permission mode
    pub fn cycle_permission_mode(&mut self) {
        self.permission_mode = self.permission_mode.next();
        self.needs_redraw = true;
    }

    /// Cycle to next agent mode (PLAN/BUILD)
    pub fn cycle_agent_mode(&mut self) {
        self.agent_mode = self.agent_mode.next();
        self.needs_redraw = true;
    }

    /// Set agent mode directly
    pub fn set_agent_mode(&mut self, mode: AgentMode) {
        self.agent_mode = mode;
        self.needs_redraw = true;
    }

    /// Set permission mode directly
    pub fn set_permission_mode(&mut self, mode: PermissionMode) {
        self.permission_mode = mode;
        self.needs_redraw = true;
    }

    /// Toggle sidebar visibility
    pub fn toggle_sidebar(&mut self) {
        self.sidebar.toggle();
        self.needs_redraw = true;
    }

    /// Update sidebar from a plan event
    pub fn update_plan(&mut self, event: &PlanEvent) {
        self.sidebar.update_from_event(event);

        // Show approval popup when plan is awaiting approval
        match event {
            PlanEvent::PlanCreated { plan } => {
                // Store the plan for potential approval display
                self.pending_approval_plan = Some(plan.clone());
            }
            PlanEvent::AwaitingApproval { .. } => {
                self.plan_approval_visible = true;
            }
            PlanEvent::PlanApproved { .. } | PlanEvent::PlanRejected { .. } => {
                self.plan_approval_visible = false;
                self.pending_approval_plan = None;
            }
            _ => {}
        }

        self.needs_redraw = true;
    }

    /// Approve the pending plan
    pub fn approve_plan(&mut self) {
        if let Some(tx) = self.plan_approval_tx.take() {
            let _ = tx.send(true);
        }
        self.plan_approval_visible = false;

        // Update sidebar to show plan is approved (no longer awaiting)
        if let Some(ref mut plan) = self.sidebar.active_plan {
            plan.awaiting_approval = false;
        }

        // Start plan execution - mark first step as in progress
        self.plan_executing = true;
        self.current_plan_step = 0;
        self.start_plan_step(0);

        self.needs_redraw = true;
    }

    /// Mark a plan step as in progress by index
    pub fn start_plan_step(&mut self, step_index: usize) {
        if let Some(ref mut plan) = self.sidebar.active_plan {
            if step_index < plan.steps.len() {
                plan.current_step_idx = Some(step_index);
                plan.steps[step_index].status = crate::planning::PlanStepStatus::InProgress;
            }
        }
        self.needs_redraw = true;
    }

    /// Mark a plan step as completed by index
    pub fn complete_plan_step(&mut self, step_index: usize, success: bool) {
        if let Some(ref mut plan) = self.sidebar.active_plan {
            if step_index < plan.steps.len() {
                plan.steps[step_index].status = if success {
                    crate::planning::PlanStepStatus::Completed
                } else {
                    crate::planning::PlanStepStatus::Failed
                };
            }
        }
        self.needs_redraw = true;
    }

    /// Get the number of plan steps
    pub fn plan_step_count(&self) -> usize {
        self.sidebar.active_plan.as_ref().map(|p| p.steps.len()).unwrap_or(0)
    }

    /// Reject the pending plan
    pub fn reject_plan(&mut self) {
        if let Some(tx) = self.plan_approval_tx.take() {
            let _ = tx.send(false);
        }
        self.plan_approval_visible = false;
        self.pending_approval_plan = None;
        self.needs_redraw = true;
    }

    /// Set the approval sender (called when session starts plan approval)
    pub fn set_plan_approval_tx(&mut self, tx: tokio::sync::mpsc::UnboundedSender<bool>) {
        self.plan_approval_tx = Some(tx);
    }

    /// Check if plan approval popup is visible
    pub fn is_plan_approval_visible(&self) -> bool {
        self.plan_approval_visible
    }

    /// Update token usage in sidebar
    pub fn update_tokens(&mut self, input: usize, output: usize) {
        self.sidebar.update_tokens(input, output);
        self.needs_redraw = true;
    }

    /// Update token usage in sidebar with cache statistics
    pub fn update_tokens_with_cache(
        &mut self,
        input: usize,
        output: usize,
        cache_read: Option<usize>,
        cache_write: Option<usize>,
    ) {
        self.sidebar
            .update_tokens_with_cache(input, output, cache_read, cache_write);
        self.needs_redraw = true;
    }

    /// Sync LSP servers to sidebar
    pub fn sync_lsp_to_sidebar(&mut self) {
        for (lang, _cmd, running) in &self.lsp_servers {
            self.sidebar.add_lsp_server(lang.clone(), *running);
        }
        self.needs_redraw = true;
    }

    /// Sync todos to sidebar for checklist display
    pub fn sync_todos_to_sidebar(&mut self) {
        use crate::tools::todo::get_todo_list;
        let todos = get_todo_list();
        self.sidebar.update_todos(&todos);
        self.needs_redraw = true;
    }

    /// Build AI context from recent shell activity
    pub fn build_ai_context(&self) -> String {
        let mut context = String::new();

        // Current environment
        context.push_str(&format!("Current directory: {}\n", self.cwd.display()));
        if let Some(branch) = self.get_git_branch() {
            context.push_str(&format!("Git branch: {}\n", branch));
        }

        // Detect and include project type
        context.push_str("\n## Project Information\n");
        if let Some(project_info) = self.detect_project_type() {
            context.push_str(&project_info);
        } else {
            context.push_str("Project type: Unknown\n");
        }

        // Recent shell commands
        context.push_str("\n## Recent shell activity:\n");

        let shell_blocks: Vec<_> = self
            .blocks
            .iter()
            .filter(|b| matches!(b.block_type, BlockType::ShellCommand))
            .collect();

        let start = shell_blocks.len().saturating_sub(self.ai_context_commands);
        for block in shell_blocks.into_iter().skip(start) {
            context.push_str(&format!("$ {}\n", block.input));

            let output = block.output.get_text();
            if !output.is_empty() {
                // Truncate long outputs (UTF-8 safe)
                let preview = if output.chars().count() > 500 {
                    let truncated: String = output.chars().take(500).collect();
                    format!("{}...[truncated]", truncated)
                } else {
                    output
                };
                context.push_str(&format!("{}\n", preview));
            }

            if let Some(code) = block.exit_code {
                if code != 0 {
                    context.push_str(&format!("[exit code: {}]\n", code));
                }
            }
            context.push('\n');
        }

        context
    }

    /// Detect project type and return relevant context
    fn detect_project_type(&self) -> Option<String> {
        let mut info = String::new();

        // Check for Rust project (Cargo.toml)
        let cargo_toml = self.cwd.join("Cargo.toml");
        if cargo_toml.exists() {
            info.push_str("Project type: Rust (Cargo)\n");
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                // Extract package name
                if let Some(name_line) = content.lines().find(|l| l.trim().starts_with("name")) {
                    if let Some(name) = name_line.split('=').nth(1) {
                        info.push_str(&format!("Package: {}\n", name.trim().trim_matches('"')));
                    }
                }
                // Extract description if available
                if let Some(desc_line) = content
                    .lines()
                    .find(|l| l.trim().starts_with("description"))
                {
                    if let Some(desc) = desc_line.split('=').nth(1) {
                        info.push_str(&format!("Description: {}\n", desc.trim().trim_matches('"')));
                    }
                }
            }
            // Check for src directory structure
            if self.cwd.join("src/main.rs").exists() {
                info.push_str("Type: Binary application\n");
            } else if self.cwd.join("src/lib.rs").exists() {
                info.push_str("Type: Library\n");
            }
            return Some(info);
        }

        // Check for Node.js project (package.json)
        let package_json = self.cwd.join("package.json");
        if package_json.exists() {
            info.push_str("Project type: Node.js/JavaScript\n");
            if let Ok(content) = std::fs::read_to_string(&package_json) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(name) = json.get("name").and_then(|v| v.as_str()) {
                        info.push_str(&format!("Package: {}\n", name));
                    }
                    if let Some(desc) = json.get("description").and_then(|v| v.as_str()) {
                        info.push_str(&format!("Description: {}\n", desc));
                    }
                    // Check for TypeScript
                    if json
                        .get("devDependencies")
                        .and_then(|d| d.get("typescript"))
                        .is_some()
                        || self.cwd.join("tsconfig.json").exists()
                    {
                        info.push_str("Language: TypeScript\n");
                    }
                }
            }
            return Some(info);
        }

        // Check for Python project
        let pyproject = self.cwd.join("pyproject.toml");
        let setup_py = self.cwd.join("setup.py");
        let requirements = self.cwd.join("requirements.txt");
        if pyproject.exists() || setup_py.exists() || requirements.exists() {
            info.push_str("Project type: Python\n");
            if pyproject.exists() {
                if let Ok(content) = std::fs::read_to_string(&pyproject) {
                    if let Some(name_line) = content.lines().find(|l| l.trim().starts_with("name"))
                    {
                        if let Some(name) = name_line.split('=').nth(1) {
                            info.push_str(&format!("Package: {}\n", name.trim().trim_matches('"')));
                        }
                    }
                }
            }
            return Some(info);
        }

        // Check for Go project
        let go_mod = self.cwd.join("go.mod");
        if go_mod.exists() {
            info.push_str("Project type: Go\n");
            if let Ok(content) = std::fs::read_to_string(&go_mod) {
                if let Some(module_line) = content.lines().find(|l| l.starts_with("module")) {
                    if let Some(module) = module_line.split_whitespace().nth(1) {
                        info.push_str(&format!("Module: {}\n", module));
                    }
                }
            }
            return Some(info);
        }

        // Check for Java/Maven project
        let pom_xml = self.cwd.join("pom.xml");
        if pom_xml.exists() {
            info.push_str("Project type: Java (Maven)\n");
            return Some(info);
        }

        // Check for Java/Gradle project
        let build_gradle = self.cwd.join("build.gradle");
        let build_gradle_kts = self.cwd.join("build.gradle.kts");
        if build_gradle.exists() || build_gradle_kts.exists() {
            info.push_str("Project type: Java/Kotlin (Gradle)\n");
            return Some(info);
        }

        None
    }

    // === Directory Management ===

    /// Change directory
    pub fn change_directory(&mut self, path: &str) -> Result<(), String> {
        let new_path = if path.is_empty() || path == "~" {
            dirs::home_dir().ok_or("Could not find home directory")?
        } else if path.starts_with('~') {
            let home = dirs::home_dir().ok_or("Could not find home directory")?;
            home.join(&path[2..])
        } else if path.starts_with('/') {
            PathBuf::from(path)
        } else {
            self.cwd.join(path)
        };

        let canonical = new_path
            .canonicalize()
            .map_err(|_| format!("cd: {}: No such directory", path))?;

        if !canonical.is_dir() {
            return Err(format!("cd: {}: Not a directory", path));
        }

        self.cwd = canonical;
        self.needs_redraw = true;
        Ok(())
    }

    /// Show the commands modal
    pub fn show_commands_modal(&mut self) {
        self.commands_modal_visible = true;
        self.needs_redraw = true;
    }

    /// Hide the commands modal
    pub fn hide_commands_modal(&mut self) {
        self.commands_modal_visible = false;
        self.needs_redraw = true;
    }

    /// Check if input is a slash command (e.g., /connect, /help)
    pub fn is_slash_command(input: &str) -> bool {
        input.starts_with('/')
    }

    /// Check if input is a built-in shell command
    pub fn is_builtin_command(input: &str) -> bool {
        let cmd = input.split_whitespace().next().unwrap_or("");
        matches!(
            cmd,
            "cd" | "pwd" | "exit" | "quit" | "clear" | "history" | "export" | "env"
        )
    }

    /// Check if input looks like a shell command (starts with common commands or contains shell operators)
    pub fn looks_like_shell_command(input: &str) -> bool {
        // Never treat slash commands as shell commands
        if input.trim().starts_with('/') {
            return false;
        }

        let first_word = input.split_whitespace().next().unwrap_or("");

        // Common shell commands
        let shell_commands = [
            "ls", "cat", "grep", "find", "mkdir", "rm", "mv", "cp", "touch", "echo", "git",
            "cargo", "npm", "yarn", "pnpm", "node", "python", "python3", "pip", "pip3", "make",
            "cmake", "gcc", "clang", "rustc", "go", "java", "javac", "ruby", "perl", "php",
            "docker", "kubectl", "curl", "wget", "ssh", "scp", "rsync", "tar", "zip", "unzip",
            "head", "tail", "less", "more", "wc", "sort", "uniq", "awk", "sed", "chmod", "chown",
            "sudo", "apt", "brew", "yum", "dnf", "pacman", "which", "where", "man", "diff",
            "patch", "tree", "du", "df", "ps", "top", "htop", "kill", "killall", "ping", "nc",
            "netstat", ".", "source", "bash", "sh", "zsh", "fish",
        ];

        // Check if starts with a known shell command
        if shell_commands.contains(&first_word) {
            return true;
        }

        // Check for shell operators/patterns
        if input.contains('|')
            || input.contains('>')
            || input.contains('<')
            || input.contains("&&")
            || input.contains("||")
            || input.starts_with("./")
            || input.starts_with("../")
            || first_word.starts_with("./")
            || first_word.starts_with("../")
        {
            return true;
        }

        // Check if it looks like a path execution
        if first_word.contains('/') && !first_word.starts_with('@') {
            return true;
        }

        false
    }

    /// Parse slash command from input (e.g., /connect, /disconnect)
    pub fn parse_slash_command(input: &str) -> Option<SlashCommand> {
        if !input.starts_with('/') {
            return None;
        }

        let rest = input[1..].trim();
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        let cmd = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
        let args = parts.get(1).map(|s| s.to_string());

        match cmd.as_str() {
            "connect" => Some(SlashCommand::Connect),
            "disconnect" => Some(SlashCommand::Disconnect),
            "orchestrate" | "orch" => Some(SlashCommand::Orchestrate(args.unwrap_or_default())),
            "help" => Some(SlashCommand::Help),
            "tools" => Some(SlashCommand::Tools),
            "mode" => Some(SlashCommand::Mode),
            "agent" => Some(SlashCommand::Agent),
            "commands" => Some(SlashCommand::Commands),
            "models" => Some(SlashCommand::Models),
            "provider" => Some(SlashCommand::Provider(args)),
            "model" => Some(SlashCommand::Model(args)),
            "login" => Some(SlashCommand::Login(args)),
            _ => None,
        }
    }

    /// Extract file context patterns from input (words starting with @)
    /// Returns (query_without_files, file_patterns)
    pub fn extract_file_context(input: &str) -> (String, Vec<String>) {
        let mut query_parts = Vec::new();
        let mut file_patterns = Vec::new();

        for word in input.split_whitespace() {
            if word.starts_with('@') && word.len() > 1 {
                // This is a file pattern
                file_patterns.push(word[1..].to_string());
            } else {
                query_parts.push(word);
            }
        }

        (query_parts.join(" "), file_patterns)
    }
}

/// Slash commands (e.g., /connect, /help)
#[derive(Debug, Clone)]
pub enum SlashCommand {
    /// Connect to AI service
    Connect,
    /// Disconnect from AI service
    Disconnect,
    /// Run orchestration task
    Orchestrate(String),
    /// Show help
    Help,
    /// List available tools
    Tools,
    /// Show/toggle permission mode
    Mode,
    /// Show/toggle agent mode (PLAN/BUILD)
    Agent,
    /// Show commands reference
    Commands,
    /// List available models for current provider
    Models,
    /// Switch or show current provider
    Provider(Option<String>),
    /// Switch or show current model
    Model(Option<String>),
    /// Login to a provider
    Login(Option<String>),
}
