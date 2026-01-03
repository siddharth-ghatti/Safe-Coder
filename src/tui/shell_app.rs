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
            agent_mode: AgentMode::default(),

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
            file_picker: FilePicker::new(),
            commands_modal_visible: false,

            needs_redraw: true,
            animation_frame: 0,
            spinner: Spinner::new(),
            start_time: Local::now(),

            lsp_servers: Vec::new(),
            lsp_status_message: None,
            lsp_initializing: true,

            sidebar: SidebarState::new(),
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
        self.needs_redraw = true;
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
            "commands" => Some(SlashCommand::Commands),
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
    /// Show commands reference
    Commands,
}
