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
use super::spinner::Spinner;
use crate::config::Config;
use crate::session::Session;

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

/// Type of command block
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
        }
    }

    /// Create a system message block
    pub fn system(message: String, prompt: ShellPrompt) -> Self {
        let mut block = Self::new(String::new(), BlockType::SystemMessage, prompt);
        block.output = BlockOutput::Success(message);
        block
    }

    /// Mark the block as completed with success
    pub fn complete(&mut self, output: String, exit_code: i32) {
        let elapsed = Local::now()
            .signed_duration_since(self.timestamp)
            .num_milliseconds() as u64;
        self.output = BlockOutput::Success(output);
        self.exit_code = Some(exit_code);
        self.duration_ms = Some(elapsed);
    }

    /// Mark the block as failed
    pub fn fail(&mut self, message: String, stderr: String, exit_code: i32) {
        let elapsed = Local::now()
            .signed_duration_since(self.timestamp)
            .num_milliseconds() as u64;
        self.output = BlockOutput::Error { message, stderr };
        self.exit_code = Some(exit_code);
        self.duration_ms = Some(elapsed);
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
    }

    /// Add a child block (for AI tool executions)
    pub fn add_child(&mut self, child: CommandBlock) {
        self.children.push(child);
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

    // === UI State ===
    /// Current input text
    pub input: String,
    /// Cursor position in input
    pub cursor_pos: usize,
    /// Current input mode
    pub input_mode: InputMode,
    /// Scroll offset for block list (0 = bottom/most recent)
    pub scroll_offset: usize,
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

    // === Animation/Render State ===
    /// Whether UI needs to be redrawn
    pub needs_redraw: bool,
    /// Animation frame counter
    pub animation_frame: usize,
    /// Spinner for AI thinking
    pub spinner: Spinner,
    /// Start time for session
    pub start_time: DateTime<Local>,
}

impl ShellTuiApp {
    /// Create a new shell TUI application
    pub fn new(project_path: PathBuf, config: Config) -> Self {
        let cwd = project_path.clone();
        let cwd_short = cwd
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "~".to_string());

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

            input: String::new(),
            cursor_pos: 0,
            input_mode: InputMode::Normal,
            scroll_offset: 0,
            selected_block: None,
            focus: FocusArea::Input,
            search_query: String::new(),
            search_results: Vec::new(),
            search_result_pos: 0,
            autocomplete: Autocomplete::new(),

            needs_redraw: true,
            animation_frame: 0,
            spinner: Spinner::new(),
            start_time: Local::now(),
        };

        // Add welcome message
        let welcome = format!(
            "Welcome to Safe Coder Shell!\n\n\
             Project: {}\n\n\
             Commands:\n\
             • Type shell commands normally (ls, git, cargo, etc.)\n\
             • @ <query>     - Ask AI for help\n\
             • @connect      - Connect to AI\n\
             • @disconnect   - Disconnect from AI\n\
             • @orchestrate  - Run multi-agent task\n\
             • exit          - Exit shell\n\n\
             Press Ctrl+C to cancel running commands.",
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

    /// Tick animation state
    pub fn tick(&mut self) {
        self.animation_frame = (self.animation_frame + 1) % 100;

        if self.ai_thinking {
            self.spinner.tick();
            self.needs_redraw = true;
        }

        // Cursor blink
        let old_cursor = (self.animation_frame.wrapping_sub(1) % 20) < 10;
        let new_cursor = (self.animation_frame % 20) < 10;
        if old_cursor != new_cursor {
            self.needs_redraw = true;
        }

        // Check for any running blocks
        if self.blocks.iter().any(|b| b.is_running()) {
            self.needs_redraw = true;
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

        self.autocomplete.complete(&self.input, &self.cwd);

        // If there's exactly one match, apply it immediately
        if self.autocomplete.single_match() {
            self.apply_autocomplete();
        }

        self.needs_redraw = true;
    }

    /// Apply the currently selected autocomplete suggestion
    pub fn apply_autocomplete(&mut self) {
        if let Some(new_input) = self.autocomplete.apply(&self.input) {
            self.input = new_input;
            self.cursor_pos = self.input.len();
            self.autocomplete.hide();
            self.needs_redraw = true;
        }
    }

    /// Move to next autocomplete suggestion
    pub fn autocomplete_next(&mut self) {
        self.autocomplete.next();
        self.needs_redraw = true;
    }

    /// Move to previous autocomplete suggestion
    pub fn autocomplete_prev(&mut self) {
        self.autocomplete.prev();
        self.needs_redraw = true;
    }

    /// Check if autocomplete is currently visible
    pub fn autocomplete_visible(&self) -> bool {
        self.autocomplete.visible
    }

    /// Update input mode based on current input
    fn update_input_mode(&mut self) {
        if self.input.starts_with('@') {
            self.input_mode = InputMode::AiPrefix;
        } else {
            self.input_mode = InputMode::Normal;
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
        self.scroll_to_bottom();
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
            self.needs_redraw = true;
        }
    }

    // === Scrolling ===

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(1);
        self.needs_redraw = true;
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
        self.needs_redraw = true;
    }

    pub fn scroll_page_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(10);
        self.needs_redraw = true;
    }

    pub fn scroll_page_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(10);
        self.needs_redraw = true;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
        self.needs_redraw = true;
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

    /// Set permission mode directly
    pub fn set_permission_mode(&mut self, mode: PermissionMode) {
        self.permission_mode = mode;
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

        // Recent shell commands
        context.push_str("\nRecent shell activity:\n");

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
                // Truncate long outputs
                let preview = if output.len() > 500 {
                    format!("{}...[truncated]", &output[..500])
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

    /// Check if input is an AI command
    pub fn is_ai_command(input: &str) -> bool {
        input.starts_with('@')
    }

    /// Check if input is a built-in command
    pub fn is_builtin_command(input: &str) -> bool {
        let cmd = input.split_whitespace().next().unwrap_or("");
        matches!(
            cmd,
            "cd" | "pwd" | "exit" | "quit" | "clear" | "history" | "export" | "env" | "help"
        )
    }

    /// Parse AI command from input
    pub fn parse_ai_command(input: &str) -> Option<AiCommand> {
        if !input.starts_with('@') {
            return None;
        }

        let rest = input[1..].trim();
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        let cmd = parts.first().map(|s| s.to_lowercase()).unwrap_or_default();
        let args = parts.get(1).map(|s| s.to_string());

        match cmd.as_str() {
            "connect" => Some(AiCommand::Connect),
            "disconnect" => Some(AiCommand::Disconnect),
            "orchestrate" | "orch" => Some(AiCommand::Orchestrate(args.unwrap_or_default())),
            "" => None, // Just "@" with nothing after
            _ => Some(AiCommand::Query(rest.to_string())),
        }
    }
}

/// AI commands parsed from @-prefixed input
#[derive(Debug, Clone)]
pub enum AiCommand {
    /// Connect to AI service
    Connect,
    /// Disconnect from AI service
    Disconnect,
    /// Run orchestration task
    Orchestrate(String),
    /// Query AI with the given text
    Query(String),
}
