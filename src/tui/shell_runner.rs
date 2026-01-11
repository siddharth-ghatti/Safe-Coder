//! Shell TUI runner - event loop and command execution
//!
//! This module handles the main event loop for the shell-first TUI,
//! including keyboard input, command execution, and AI integration.

use anyhow::{Context, Result};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::FutureExt;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, BufRead, BufReader};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader as TokioBufReader};
use tokio::process::Command as TokioCommand;
use tokio::sync::{mpsc, Mutex};

use super::shell_app::{BlockOutput, BlockType, CommandBlock, FileDiff, ShellTuiApp, SlashCommand};
use super::shell_ui;
use crate::config::Config;
use crate::llm::create_client;
use crate::lsp::{default_lsp_configs, LspClient, LspManager};
use crate::orchestrator::TaskPlan;
use crate::planning::PlanEvent;
use crate::session::{Session, SessionEvent};
use crate::unified_planning::{ExecutionMode, UnifiedPlanner};

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
    /// AI is thinking/processing
    Thinking { block_id: String, message: String },
    /// AI reasoning text (explanation before/between tool calls)
    Reasoning { block_id: String, text: String },
    /// AI response text chunk
    TextChunk { block_id: String, text: String },
    /// AI final response
    Response { block_id: String, text: String },
    /// AI started tool execution
    ToolStart {
        block_id: String,
        tool_name: String,
        description: String,
    },
    /// Tool produced output
    ToolOutput {
        block_id: String,
        tool_name: String,
        output: String,
    },
    /// Streaming bash output line (for inline display)
    BashOutputLine {
        block_id: String,
        tool_name: String,
        line: String,
    },
    /// AI tool completed
    ToolComplete {
        block_id: String,
        tool_name: String,
        success: bool,
    },
    /// File diff from edit operation
    FileDiff {
        block_id: String,
        path: String,
        old_content: String,
        new_content: String,
    },
    /// AI processing complete
    Complete { block_id: String },
    /// AI error
    Error { block_id: String, message: String },
    /// Plan event for sidebar updates
    PlanEvent { block_id: String, event: PlanEvent },
    /// Token usage update
    TokenUsage {
        block_id: String,
        input_tokens: usize,
        output_tokens: usize,
        cache_read_tokens: Option<usize>,
        cache_creation_tokens: Option<usize>,
    },
    /// Context was compressed
    ContextCompressed {
        block_id: String,
        tokens_compressed: usize,
    },
    /// Plan approval sender (for TUI to respond to plan approval)
    PlanApprovalSender(tokio::sync::mpsc::UnboundedSender<bool>),
}

/// Message types for orchestration updates
#[derive(Debug)]
enum OrchestrationUpdate {
    /// Planning phase started
    Planning { block_id: String },
    /// Plan created and ready for review
    PlanReady { block_id: String, plan: TaskPlan },
    /// Plan approved and execution starting
    Executing { block_id: String, task_count: usize },
    /// Task started
    TaskStarted {
        block_id: String,
        task_id: String,
        description: String,
        worker: String,
    },
    /// Streaming output line from task
    TaskOutput {
        block_id: String,
        task_id: String,
        line: String,
    },
    /// Task completed
    TaskCompleted {
        block_id: String,
        task_id: String,
        success: bool,
        output: String,
    },
    /// Orchestration complete
    Complete {
        block_id: String,
        summary: String,
        success_count: usize,
        fail_count: usize,
    },
    /// Orchestration error
    Error { block_id: String, message: String },
}

/// Shell TUI runner
pub struct ShellTuiRunner {
    app: ShellTuiApp,
    config: Config,
    lsp_manager: Option<LspManager>,
}

impl ShellTuiRunner {
    /// Create a new shell TUI runner
    pub fn new(project_path: PathBuf, config: Config) -> Self {
        let mut app = ShellTuiApp::new(project_path.clone(), config.clone());

        // Initialize LSP servers info for display
        let lsp_configs = default_lsp_configs();
        for (lang, server_config) in &lsp_configs {
            let available = which::which(&server_config.command).is_ok();
            if available {
                app.lsp_servers.push((
                    lang.clone(),
                    server_config.command.clone(),
                    false, // Not running yet
                ));
            }
        }

        Self {
            app,
            config,
            lsp_manager: None,
        }
    }

    /// Initialize LSP servers (runs in background, non-blocking)
    fn spawn_lsp_init(&self) -> tokio::task::JoinHandle<Option<LspManager>> {
        if !self.config.lsp.enabled {
            return tokio::spawn(async { None });
        }

        let project_path = self.app.project_path.clone();

        tokio::spawn(async move {
            let mut manager = LspManager::new(project_path, None);

            // Use a timeout to prevent blocking forever on downloads
            match tokio::time::timeout(std::time::Duration::from_secs(30), manager.initialize())
                .await
            {
                Ok(Ok(())) => Some(manager),
                Ok(Err(e)) => {
                    // LSP init failed but don't block CLI
                    eprintln!("LSP initialization error (continuing without LSP): {}", e);
                    Some(manager) // Return manager anyway, some servers may have started
                }
                Err(_) => {
                    // Timeout - LSP init took too long
                    eprintln!("LSP initialization timed out (continuing without LSP)");
                    Some(manager) // Return manager anyway, some servers may have started
                }
            }
        })
    }

    /// Run the shell TUI
    pub async fn run(&mut self) -> Result<()> {
        // Spawn LSP initialization in background (non-blocking)
        let lsp_handle = self.spawn_lsp_init();

        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Run the event loop, passing LSP handle for background completion
        let result = self.run_event_loop(&mut terminal, lsp_handle).await;

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
        mut lsp_handle: tokio::task::JoinHandle<Option<LspManager>>,
    ) -> Result<()> {
        // Channels for async command execution
        let (cmd_tx, mut cmd_rx) = mpsc::unbounded_channel::<CommandUpdate>();
        let (ai_tx, mut ai_rx) = mpsc::unbounded_channel::<AiUpdate>();
        let (orch_tx, mut orch_rx) = mpsc::unbounded_channel::<OrchestrationUpdate>();

        // Track whether LSP initialization is complete
        let mut lsp_init_complete = false;

        loop {
            // Check if LSP initialization completed (non-blocking)
            if !lsp_init_complete {
                if let Some(result) = (&mut lsp_handle).now_or_never() {
                    lsp_init_complete = true;
                    self.app.lsp_initializing = false;

                    match result {
                        Ok(Some(manager)) => {
                            // Rebuild LSP status from actually running servers
                            // This handles both pre-installed and auto-downloaded servers
                            self.app.lsp_servers.clear();
                            for (lang, client) in manager.get_clients() {
                                if client.is_running() {
                                    self.app.lsp_servers.push((
                                        lang.clone(),
                                        client.command().to_string(),
                                        true,
                                    ));
                                    // Also update sidebar
                                    self.app.sidebar.add_lsp_server(lang.clone(), true);
                                }
                            }

                            // Set status message based on results
                            if self.app.lsp_servers.is_empty() {
                                self.app.lsp_status_message =
                                    Some("LSP: no servers started".to_string());
                            } else {
                                // Clear any error message - status bar will show running servers
                                self.app.lsp_status_message = None;
                            }

                            self.lsp_manager = Some(manager);
                        }
                        Ok(None) => {
                            // LSP disabled or failed completely
                            if self.config.lsp.enabled {
                                self.app.lsp_status_message = Some("LSP: init failed".to_string());
                            } else {
                                self.app.lsp_status_message = Some("LSP: disabled".to_string());
                            }
                        }
                        Err(_) => {
                            // Task panicked
                            self.app.lsp_status_message = Some("LSP: init error".to_string());
                        }
                    }
                    self.app.mark_dirty();
                }
            }
            // Draw if needed
            if self.app.needs_redraw {
                terminal.draw(|f| shell_ui::draw(f, &mut self.app))?;
                self.app.clear_dirty();
            }

            // Poll for events (16ms = ~60fps for smooth scrolling)
            if event::poll(Duration::from_millis(16))? {
                match event::read()? {
                    Event::Key(key) => {
                        match self
                            .handle_key_event(key.code, key.modifiers, &cmd_tx, &ai_tx, &orch_tx)
                            .await
                        {
                            Ok(true) => break, // Exit requested
                            Ok(false) => {}
                            Err(e) => {
                                // Show error in UI
                                let prompt = self.app.current_prompt();
                                let block = CommandBlock::system(format!("Error: {}", e), prompt);
                                self.app.add_block(block);
                            }
                        }
                    }
                    Event::Mouse(mouse) => match mouse.kind {
                        MouseEventKind::ScrollUp => {
                            self.app.scroll_up();
                        }
                        MouseEventKind::ScrollDown => {
                            self.app.scroll_down();
                        }
                        _ => {}
                    },
                    _ => {}
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
                    AiUpdate::Thinking {
                        block_id,
                        message: _,
                    } => {
                        // Don't set output - let the block remain in "running" state
                        // which shows the animated spinner via MessageLine::Running
                        if let Some(block) = self.app.get_block_mut(&block_id) {
                            // Keep output empty so the "Thinking..." spinner shows
                            block.output = BlockOutput::Streaming {
                                lines: vec![],
                                complete: false,
                            };
                        }
                        self.app.mark_dirty();
                    }
                    AiUpdate::Reasoning { block_id, text } => {
                        // Skip empty reasoning
                        if text.trim().is_empty() {
                            continue;
                        }

                        // Show reasoning inline as streaming output with a thinking prefix
                        // This is more visible than child blocks
                        if let Some(block) = self.app.get_block_mut(&block_id) {
                            // Format reasoning with a thinking indicator
                            let formatted_lines: Vec<String> = text
                                .lines()
                                .filter(|l| !l.trim().is_empty())
                                .map(|l| format!("💭 {}", l))
                                .collect();

                            match &mut block.output {
                                BlockOutput::Streaming { lines, .. } => {
                                    // Add reasoning lines to streaming output
                                    lines.extend(formatted_lines);
                                }
                                BlockOutput::Pending => {
                                    block.output = BlockOutput::Streaming {
                                        lines: formatted_lines,
                                        complete: false,
                                    };
                                }
                                _ => {
                                    // If already has output, prepend reasoning
                                    let existing = block.output.get_text();
                                    let reasoning_text = formatted_lines.join("\n");
                                    block.output = BlockOutput::Streaming {
                                        lines: vec![reasoning_text, existing],
                                        complete: false,
                                    };
                                }
                            }
                        }
                        self.app.mark_dirty();
                    }
                    AiUpdate::TextChunk { block_id, text } => {
                        // Skip empty text
                        if text.trim().is_empty() {
                            continue;
                        }

                        // Check if tools have been executed (any tool children exist)
                        // If so, add text as inline reasoning between tools
                        // If not, just show as streaming (will be replaced by final Response)
                        let has_tool_children = self
                            .app
                            .get_block_mut(&block_id)
                            .map(|b| {
                                b.children.iter().any(|c| {
                                    matches!(c.block_type, BlockType::AiToolExecution { .. })
                                })
                            })
                            .unwrap_or(false);

                        if has_tool_children {
                            // Add as reasoning child block (inline between/after tools)
                            let prompt = self.app.current_prompt();
                            let mut reasoning_block =
                                CommandBlock::new(String::new(), BlockType::AiReasoning, prompt);
                            reasoning_block.output = BlockOutput::Success(text);
                            reasoning_block.exit_code = Some(0);

                            if let Some(parent) = self.app.get_block_mut(&block_id) {
                                parent.add_child(reasoning_block);
                            }
                        } else {
                            // No tools yet - show as streaming preview
                            // This will be replaced by final Response
                            if let Some(block) = self.app.get_block_mut(&block_id) {
                                match &mut block.output {
                                    BlockOutput::Streaming { lines, .. } => {
                                        // Replace thinking message with actual text
                                        if lines.len() == 1 && lines[0].starts_with("💭") {
                                            lines.clear();
                                        }
                                        for line in text.lines() {
                                            lines.push(line.to_string());
                                        }
                                    }
                                    BlockOutput::Pending => {
                                        block.output = BlockOutput::Streaming {
                                            lines: text.lines().map(|s| s.to_string()).collect(),
                                            complete: false,
                                        };
                                    }
                                    _ => {}
                                }
                            }
                        }
                        self.app.mark_dirty();
                    }
                    AiUpdate::Response { block_id, text } => {
                        if let Some(block) = self.app.get_block_mut(&block_id) {
                            block.output = BlockOutput::Success(text);
                        }
                        self.app.mark_dirty();
                    }
                    AiUpdate::ToolStart {
                        block_id,
                        tool_name,
                        description,
                    } => {
                        // Track tool step in sidebar if in build mode
                        if self.app.agent_mode == crate::tools::AgentMode::Build {
                            self.app
                                .sidebar
                                .add_tool_step(tool_name.clone(), description.clone());
                        }

                        // Get prompt first before mutable borrow
                        let prompt = self.app.current_prompt();
                        let child = CommandBlock::new(
                            description,
                            BlockType::AiToolExecution {
                                tool_name: tool_name.clone(),
                            },
                            prompt,
                        );
                        if let Some(parent) = self.app.get_block_mut(&block_id) {
                            parent.add_child(child);
                        }
                        self.app.mark_dirty();
                    }
                    AiUpdate::ToolOutput {
                        block_id,
                        tool_name,
                        output,
                    } => {
                        // Find the tool's child block and update its output
                        if let Some(parent) = self.app.get_block_mut(&block_id) {
                            if let Some(child) = parent.children.iter_mut().rev().find(|c| {
                                matches!(&c.block_type, BlockType::AiToolExecution { tool_name: n } if n == &tool_name)
                            }) {
                                // Truncate output for display (UTF-8 safe)
                                let display_output = if output.chars().count() > 500 {
                                    let truncated: String = output.chars().take(500).collect();
                                    format!("{}...[truncated]", truncated)
                                } else {
                                    output
                                };
                                child.output = BlockOutput::Success(display_output);
                            }
                        }
                        self.app.mark_dirty();
                    }
                    AiUpdate::BashOutputLine {
                        block_id,
                        tool_name,
                        line,
                    } => {
                        // Stream bash output inline to the tool's child block
                        if let Some(parent) = self.app.get_block_mut(&block_id) {
                            if let Some(child) = parent.children.iter_mut().rev().find(|c| {
                                matches!(&c.block_type, BlockType::AiToolExecution { tool_name: n } if n == &tool_name)
                            }) {
                                // Append line to streaming output
                                match &mut child.output {
                                    BlockOutput::Streaming { lines, .. } => {
                                        lines.push(line);
                                    }
                                    BlockOutput::Pending => {
                                        child.output = BlockOutput::Streaming {
                                            lines: vec![line],
                                            complete: false,
                                        };
                                    }
                                    _ => {
                                        // If already complete, convert to streaming and add
                                        let existing = child.output.get_text();
                                        let mut lines: Vec<String> = existing.lines().map(|s| s.to_string()).collect();
                                        lines.push(line);
                                        child.output = BlockOutput::Streaming {
                                            lines,
                                            complete: false,
                                        };
                                    }
                                }
                            }
                        }
                        self.app.mark_dirty();
                    }
                    AiUpdate::ToolComplete {
                        block_id,
                        tool_name,
                        success,
                    } => {
                        // Complete tool step in sidebar if in build mode
                        if self.app.agent_mode == crate::tools::AgentMode::Build {
                            self.app.sidebar.complete_tool_step(&tool_name, success);

                            // Update plan step progress for file-modifying tools
                            if self.app.plan_executing && success {
                                let is_file_tool = tool_name == "edit_file"
                                    || tool_name == "write_file"
                                    || tool_name == "Edit"
                                    || tool_name == "Write";

                                if is_file_tool {
                                    // Complete current step and move to next
                                    let current = self.app.current_plan_step;
                                    let total = self.app.plan_step_count();

                                    self.app.complete_plan_step(current, true);

                                    if current + 1 < total {
                                        self.app.current_plan_step = current + 1;
                                        self.app.start_plan_step(current + 1);
                                    } else {
                                        // Plan complete
                                        self.app.plan_executing = false;
                                    }
                                }
                            }
                        }

                        // Mark the tool block as complete and calculate duration
                        if let Some(parent) = self.app.get_block_mut(&block_id) {
                            if let Some(child) = parent.children.iter_mut().rev().find(|c| {
                                matches!(&c.block_type, BlockType::AiToolExecution { tool_name: n } if n == &tool_name)
                            }) {
                                child.exit_code = Some(if success { 0 } else { 1 });
                                // Calculate duration from block timestamp
                                let elapsed = chrono::Local::now() - child.timestamp;
                                child.duration_ms = Some(elapsed.num_milliseconds().max(0) as u64);
                            }
                        }
                        // If todowrite tool completed, sync todos to sidebar
                        if tool_name == "todowrite" && success {
                            self.app.sync_todos_to_sidebar();
                        }
                        self.app.mark_dirty();
                    }
                    AiUpdate::FileDiff {
                        block_id,
                        path,
                        old_content,
                        new_content,
                    } => {
                        // Track file modification in sidebar
                        use crate::tui::sidebar::ModificationType;
                        let mod_type = if old_content.is_empty() {
                            ModificationType::Created
                        } else {
                            ModificationType::Edited
                        };
                        self.app
                            .sidebar
                            .track_file_modification(path.clone(), mod_type);

                        // Store diff in the most recent tool child block
                        if let Some(parent) = self.app.get_block_mut(&block_id) {
                            if let Some(child) = parent.children.last_mut() {
                                child.diff = Some(FileDiff {
                                    path,
                                    old_content,
                                    new_content,
                                });
                            }
                        }
                        self.app.mark_dirty();
                    }
                    AiUpdate::Complete { block_id } => {
                        if let Some(block) = self.app.get_block_mut(&block_id) {
                            // Convert streaming to success if needed
                            if let BlockOutput::Streaming { lines, .. } = &block.output {
                                block.output = BlockOutput::Success(lines.join("\n"));
                            } else if matches!(block.output, BlockOutput::Pending) {
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
                    AiUpdate::PlanEvent { event, .. } => {
                        // Update sidebar with plan event
                        self.app.update_plan(&event);
                    }
                    AiUpdate::PlanApprovalSender(tx) => {
                        // Store approval sender for TUI to use when user approves/rejects
                        self.app.set_plan_approval_tx(tx);
                    }
                    AiUpdate::TokenUsage {
                        input_tokens,
                        output_tokens,
                        cache_read_tokens,
                        cache_creation_tokens,
                        ..
                    } => {
                        // Update token usage in sidebar with cache stats
                        self.app.update_tokens_with_cache(
                            input_tokens,
                            output_tokens,
                            cache_read_tokens,
                            cache_creation_tokens,
                        );
                    }
                    AiUpdate::ContextCompressed {
                        tokens_compressed, ..
                    } => {
                        // Record compressed tokens in sidebar
                        self.app
                            .sidebar
                            .token_usage
                            .record_compression(tokens_compressed);
                        self.app.mark_dirty();
                    }
                }
            }

            // Process orchestration updates
            while let Ok(update) = orch_rx.try_recv() {
                match update {
                    OrchestrationUpdate::Planning { block_id } => {
                        if let Some(block) = self.app.get_block_mut(&block_id) {
                            block.output = BlockOutput::Streaming {
                                lines: vec!["🎯 Creating orchestration plan...".to_string()],
                                complete: false,
                            };
                        }
                        self.app.mark_dirty();
                    }
                    OrchestrationUpdate::PlanReady { block_id, plan } => {
                        // Add child blocks for each task in the plan
                        let prompt = self.app.current_prompt();
                        if let Some(parent) = self.app.get_block_mut(&block_id) {
                            parent.output = BlockOutput::Streaming {
                                lines: vec![
                                    format!("📋 Plan: {}", plan.summary),
                                    format!("   {} task(s) to execute", plan.tasks.len()),
                                ],
                                complete: false,
                            };

                            // Add a child block showing plan details
                            let mut plan_block = CommandBlock::new(
                                "Plan Details".to_string(),
                                BlockType::AiToolExecution {
                                    tool_name: "plan".to_string(),
                                },
                                prompt,
                            );
                            let mut plan_text = String::new();
                            for (i, task) in plan.tasks.iter().enumerate() {
                                plan_text.push_str(&format!(
                                    "{}. {} [{}]\n   {}\n",
                                    i + 1,
                                    task.description,
                                    task.preferred_worker
                                        .as_ref()
                                        .map(|w| format!("{:?}", w))
                                        .unwrap_or_else(|| "default".to_string()),
                                    if task.relevant_files.is_empty() {
                                        String::new()
                                    } else {
                                        format!("Files: {}", task.relevant_files.join(", "))
                                    }
                                ));
                            }
                            plan_block.output = BlockOutput::Success(plan_text);
                            plan_block.exit_code = Some(0);
                            parent.add_child(plan_block);
                        }
                        self.app.mark_dirty();
                    }
                    OrchestrationUpdate::Executing {
                        block_id,
                        task_count,
                    } => {
                        if let Some(block) = self.app.get_block_mut(&block_id) {
                            if let BlockOutput::Streaming { lines, .. } = &mut block.output {
                                lines.push(format!("🚀 Executing {} task(s)...", task_count));
                            }
                        }
                        self.app.mark_dirty();
                    }
                    OrchestrationUpdate::TaskStarted {
                        block_id,
                        task_id,
                        description,
                        worker,
                    } => {
                        let prompt = self.app.current_prompt();
                        // Get worker icon based on worker type
                        let worker_icon = match worker.to_lowercase().as_str() {
                            "claude" | "claude-code" | "claudecode" => "🤖",
                            "gemini" | "gemini-cli" | "geminicli" => "✨",
                            "safe-coder" | "safecoder" => "🛡️",
                            "github-copilot" | "copilot" | "githubcopilot" => "🐙",
                            _ => "⚡",
                        };
                        if let Some(parent) = self.app.get_block_mut(&block_id) {
                            let mut child = CommandBlock::new(
                                format!("{} Task {}: {}", worker_icon, task_id, description),
                                BlockType::AiToolExecution {
                                    tool_name: format!("task-{}", task_id),
                                },
                                prompt,
                            );
                            // Start with streaming output
                            child.output = BlockOutput::Streaming {
                                lines: vec![],
                                complete: false,
                            };
                            parent.add_child(child);
                        }
                        self.app.mark_dirty();
                    }
                    OrchestrationUpdate::TaskOutput {
                        block_id,
                        task_id,
                        line,
                    } => {
                        // Stream output to the task's child block
                        if let Some(parent) = self.app.get_block_mut(&block_id) {
                            if let Some(child) = parent.children.iter_mut().rev().find(|c| {
                                matches!(&c.block_type, BlockType::AiToolExecution { tool_name }
                                    if tool_name == &format!("task-{}", task_id))
                            }) {
                                match &mut child.output {
                                    BlockOutput::Streaming { lines, .. } => {
                                        lines.push(line);
                                    }
                                    BlockOutput::Pending => {
                                        child.output = BlockOutput::Streaming {
                                            lines: vec![line],
                                            complete: false,
                                        };
                                    }
                                    _ => {
                                        // Convert to streaming if needed
                                        let existing = child.output.get_text();
                                        let mut lines: Vec<String> =
                                            existing.lines().map(|s| s.to_string()).collect();
                                        lines.push(line);
                                        child.output = BlockOutput::Streaming {
                                            lines,
                                            complete: false,
                                        };
                                    }
                                }
                            }
                        }
                        self.app.mark_dirty();
                    }
                    OrchestrationUpdate::TaskCompleted {
                        block_id,
                        task_id,
                        success,
                        output,
                    } => {
                        if let Some(parent) = self.app.get_block_mut(&block_id) {
                            if let Some(child) = parent.children.iter_mut().rev().find(|c| {
                                matches!(&c.block_type, BlockType::AiToolExecution { tool_name }
                                    if tool_name == &format!("task-{}", task_id))
                            }) {
                                let status = if success { "✓" } else { "✗" };
                                let truncated_output = if output.len() > 200 {
                                    format!("{}...", &output[..200])
                                } else {
                                    output
                                };
                                child.output = BlockOutput::Success(format!(
                                    "{} {}",
                                    status, truncated_output
                                ));
                                child.exit_code = Some(if success { 0 } else { 1 });
                            }
                        }
                        self.app.mark_dirty();
                    }
                    OrchestrationUpdate::Complete {
                        block_id,
                        summary,
                        success_count,
                        fail_count,
                    } => {
                        if let Some(block) = self.app.get_block_mut(&block_id) {
                            let status = if fail_count == 0 { "✅" } else { "⚠️" };
                            block.output = BlockOutput::Success(format!(
                                "{} Orchestration complete: {} succeeded, {} failed\n\n{}",
                                status, success_count, fail_count, summary
                            ));
                            block.exit_code = Some(if fail_count == 0 { 0 } else { 1 });
                        }
                        self.app.mark_dirty();
                    }
                    OrchestrationUpdate::Error { block_id, message } => {
                        self.app.fail_block(&block_id, message, String::new(), 1);
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
        orch_tx: &mpsc::UnboundedSender<OrchestrationUpdate>,
    ) -> Result<bool> {
        // Handle commands modal first
        if self.app.commands_modal_visible {
            match code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => {
                    self.app.hide_commands_modal();
                    return Ok(false);
                }
                _ => {
                    // Any other key also closes the modal
                    self.app.hide_commands_modal();
                    return Ok(false);
                }
            }
        }

        // Handle tool approval modal (highest priority)
        if self.app.has_pending_tool_approval() {
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    // Approve this tool
                    self.app.approve_pending_tool();
                    return Ok(false);
                }
                KeyCode::Char('a') | KeyCode::Char('A') => {
                    // Approve all - switch to YOLO mode and approve
                    self.app.set_permission_mode(super::shell_app::PermissionMode::Yolo);
                    self.app.approve_pending_tool();

                    // Show feedback
                    let prompt = self.app.current_prompt();
                    let block = CommandBlock::system(
                        "Switched to YOLO mode - all future actions auto-approved".to_string(),
                        prompt,
                    );
                    self.app.add_block(block);

                    return Ok(false);
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    // Deny the tool
                    self.app.deny_pending_tool();

                    // Show feedback
                    let prompt = self.app.current_prompt();
                    let block = CommandBlock::system(
                        "Tool execution denied".to_string(),
                        prompt,
                    );
                    self.app.add_block(block);

                    return Ok(false);
                }
                _ => return Ok(false), // Ignore other keys
            }
        }

        // Handle plan approval popup
        if self.app.is_plan_approval_visible() {
            match code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    // Switch to build mode and approve
                    self.app.set_agent_mode(crate::tools::AgentMode::Build);
                    self.app.approve_plan();

                    // Sync with session and trigger execution
                    if let Some(session) = &self.app.session {
                        let session = session.clone();
                        tokio::spawn(async move {
                            let mut session = session.lock().await;
                            session.set_agent_mode(crate::tools::AgentMode::Build);
                        });
                    }

                    // Trigger AI to execute the plan by sending a message
                    self.execute_ai_query(
                        "The plan has been approved. Execute it now step by step.",
                        ai_tx.clone(),
                    )
                    .await?;

                    return Ok(false);
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    // Show feedback message
                    let prompt = self.app.current_prompt();
                    let block = CommandBlock::system(
                        "❌ Plan rejected. You can modify your request and try again.".to_string(),
                        prompt,
                    );
                    self.app.add_block(block);

                    self.app.reject_plan();
                    return Ok(false);
                }
                _ => {
                    // Ignore other keys while popup is visible
                    return Ok(false);
                }
            }
        }

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

            // Ctrl+P - cycle permission mode (YOLO/EDIT/ASK)
            KeyCode::Char('p') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.app.cycle_permission_mode();
                // Show feedback
                let mode = self.app.permission_mode;
                let prompt = self.app.current_prompt();
                let block = CommandBlock::system(
                    format!(
                        "Permission mode: {} - {}",
                        mode.short_name(),
                        mode.description()
                    ),
                    prompt,
                );
                self.app.add_block(block);
            }

            // Ctrl+G - cycle agent mode (PLAN/BUILD)
            KeyCode::Char('g') if modifiers.contains(KeyModifiers::CONTROL) => {
                let _old_mode = self.app.agent_mode;
                self.app.cycle_agent_mode();
                let new_mode = self.app.agent_mode;
                let prompt = self.app.current_prompt();

                // Show prominent mode switch feedback
                let message = if new_mode == crate::tools::AgentMode::Plan {
                    format!(
                        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\
                         🔍 PLAN MODE ACTIVATED\n\
                         ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\
                         • Read-only exploration mode\n\
                         • AI will analyze and create plans\n\
                         • No files will be modified\n\
                         • Type 'approve' or press Ctrl+G to switch to BUILD mode"
                    )
                } else {
                    // Switching to BUILD mode
                    let pending_plan_msg = if self.app.pending_approval_plan.is_some() {
                        "\n• Ready to execute pending plan!"
                    } else {
                        ""
                    };
                    format!(
                        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\
                         🔨 BUILD MODE ACTIVATED\n\
                         ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\
                         • Full execution mode\n\
                         • AI can modify files and run commands\n\
                         • Press Ctrl+G to switch to PLAN mode{}",
                        pending_plan_msg
                    )
                };

                let block = CommandBlock::system(message, prompt);
                self.app.add_block(block);

                // Sync agent mode with session if connected
                if let Some(session) = &self.app.session {
                    let session = session.clone();
                    let agent_mode = new_mode;
                    tokio::spawn(async move {
                        let mut session = session.lock().await;
                        session.set_agent_mode(agent_mode);
                    });
                }
            }

            // Ctrl+B - toggle sidebar visibility
            KeyCode::Char('b') if modifiers.contains(KeyModifiers::CONTROL) => {
                self.app.toggle_sidebar();
            }

            // Ctrl+Shift+V - clear attached images
            KeyCode::Char('V') if modifiers.contains(KeyModifiers::CONTROL) && modifiers.contains(KeyModifiers::SHIFT) => {
                if self.app.has_attached_images() {
                    self.app.clear_attached_images();
                    let prompt = self.app.current_prompt();
                    let block = CommandBlock::system(
                        "📎 Attached images cleared".to_string(),
                        prompt,
                    );
                    self.app.add_block(block);
                }
            }

            // Ctrl+V - paste (check for images first)
            KeyCode::Char('v') if modifiers.contains(KeyModifiers::CONTROL) => {
                // Try to paste an image from clipboard
                match self.app.paste_image_from_clipboard() {
                    Ok(true) => {
                        // Image pasted successfully
                        let count = self.app.attached_images.len();
                        let size = self.app.attached_images_size_display();
                        let prompt = self.app.current_prompt();
                        let block = CommandBlock::system(
                            format!("📎 Image pasted ({} image{}, {})", count, if count == 1 { "" } else { "s" }, size),
                            prompt,
                        );
                        self.app.add_block(block);
                    }
                    Ok(false) => {
                        // No image in clipboard, try to paste text
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            if let Ok(text) = clipboard.get_text() {
                                // Insert text at cursor position
                                for c in text.chars() {
                                    // Skip newlines - paste as single line
                                    if c != '\n' && c != '\r' {
                                        self.app.input_push(c);
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Clipboard access failed: {}", e);
                    }
                }
            }

            // Regular character input
            KeyCode::Char(c) => {
                // Check if file picker is visible - send input there
                if self.app.file_picker.visible {
                    self.app.file_picker.filter_push(c);
                    self.app.mark_dirty();
                } else {
                    self.app.input_push(c);

                    // Trigger file picker when @ is typed
                    if c == '@' {
                        self.app.file_picker.open(&self.app.cwd);
                        self.app.mark_dirty();
                    }
                }
            }

            // Backspace
            KeyCode::Backspace => {
                if self.app.file_picker.visible {
                    if self.app.file_picker.filter.is_empty() {
                        // Close picker and remove the @ from input
                        self.app.file_picker.close();
                        self.app.input_pop(); // Remove the @
                    } else {
                        self.app.file_picker.filter_pop();
                    }
                    self.app.mark_dirty();
                } else {
                    self.app.input_pop();
                }
            }

            // Delete
            KeyCode::Delete => {
                self.app.input_delete();
            }

            // Tab - autocomplete
            KeyCode::Tab => {
                if self.app.autocomplete_visible() {
                    // Cycle through suggestions or apply
                    if modifiers.contains(KeyModifiers::SHIFT) {
                        self.app.autocomplete_prev();
                    } else {
                        self.app.autocomplete_next();
                    }
                } else {
                    // Trigger autocomplete
                    self.app.trigger_autocomplete();
                }
            }

            // Arrow keys
            KeyCode::Left => {
                if self.app.file_picker.visible {
                    // Left arrow does nothing in file picker
                } else if self.app.autocomplete_visible() {
                    // Hide any autocomplete and move cursor
                    self.app.autocomplete.hide();
                    self.app.command_autocomplete.hide();
                    self.app.cursor_left();
                } else {
                    self.app.cursor_left();
                }
            }
            KeyCode::Right => {
                if self.app.file_picker.visible {
                    // Right arrow does nothing in file picker
                } else if self.app.autocomplete_visible() {
                    self.app.apply_autocomplete();
                } else {
                    self.app.cursor_right();
                }
            }
            KeyCode::Up => {
                if self.app.file_picker.visible {
                    self.app.file_picker.select_up();
                    self.app.mark_dirty();
                } else if modifiers.contains(KeyModifiers::ALT) {
                    // Alt+Up scrolls sidebar steps up (towards older steps) in build mode
                    if self.app.agent_mode == crate::tools::AgentMode::Build {
                        self.app.sidebar.scroll_tool_steps_up();
                        self.app.mark_dirty();
                    }
                } else if modifiers.contains(KeyModifiers::SHIFT) {
                    // Shift+Up scrolls up
                    self.app.scroll_up();
                } else if self.app.autocomplete_visible() {
                    self.app.autocomplete_prev();
                } else {
                    self.app.history_up();
                }
            }
            KeyCode::Down => {
                if self.app.file_picker.visible {
                    self.app.file_picker.select_down();
                    self.app.mark_dirty();
                } else if modifiers.contains(KeyModifiers::ALT) {
                    // Alt+Down scrolls sidebar steps down (towards newer steps) in build mode
                    if self.app.agent_mode == crate::tools::AgentMode::Build {
                        self.app.sidebar.scroll_tool_steps_down();
                        self.app.mark_dirty();
                    }
                } else if modifiers.contains(KeyModifiers::SHIFT) {
                    // Shift+Down scrolls down
                    self.app.scroll_down();
                } else if self.app.autocomplete_visible() {
                    self.app.autocomplete_next();
                } else {
                    self.app.history_down();
                }
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

            // Enter - submit command or apply autocomplete/file picker
            KeyCode::Enter => {
                if self.app.file_picker.visible {
                    // Select file from picker
                    let cwd = self.app.cwd.clone();
                    if let Some(selected_path) = self.app.file_picker.select_current(&cwd) {
                        // Remove the @ we typed and add the full file reference
                        // Find the last @ in input and replace from there
                        if let Some(at_pos) = self.app.input.rfind('@') {
                            self.app.input.truncate(at_pos);
                            self.app.input.push_str(&format!("@{} ", selected_path));
                            self.app.cursor_pos = self.app.input.len();
                        }
                    }
                    self.app.mark_dirty();
                } else if self.app.autocomplete_visible() {
                    // Apply autocomplete selection
                    self.app.apply_autocomplete();
                } else {
                    let input = self.app.input_submit();
                    if !input.is_empty() {
                        self.execute_input(&input, cmd_tx.clone(), ai_tx.clone(), orch_tx.clone())
                            .await?;
                    }
                }
            }

            // Escape - cancel file picker, autocomplete, or clear input
            KeyCode::Esc => {
                if self.app.file_picker.visible {
                    self.app.file_picker.close();
                    // Also remove the @ that triggered the picker
                    if self.app.input.ends_with('@') {
                        self.app.input_pop();
                    }
                    self.app.mark_dirty();
                } else if self.app.autocomplete_visible() {
                    self.app.autocomplete.hide();
                    self.app.mark_dirty();
                } else {
                    self.app.input_clear();
                }
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
        orch_tx: mpsc::UnboundedSender<OrchestrationUpdate>,
    ) -> Result<()> {
        let input = input.trim();
        let input_lower = input.to_lowercase();

        // Check for plan approval commands when a plan is pending
        if self.app.plan_approval_tx.is_some() || self.app.pending_approval_plan.is_some() {
            if input_lower == "approve" || input_lower == "yes" || input_lower == "y" {
                // Switch to build mode and approve
                self.app.set_agent_mode(crate::tools::AgentMode::Build);
                self.app.approve_plan();

                // Sync with session
                if let Some(session) = &self.app.session {
                    let session = session.clone();
                    tokio::spawn(async move {
                        let mut session = session.lock().await;
                        session.set_agent_mode(crate::tools::AgentMode::Build);
                    });
                }

                // Trigger AI to execute the plan
                self.execute_ai_query(
                    "The plan has been approved. Execute it now step by step.",
                    ai_tx.clone(),
                )
                .await?;

                return Ok(());
            } else if input_lower == "reject" || input_lower == "no" || input_lower == "n" {
                let prompt = self.app.current_prompt();
                let block = CommandBlock::system(
                    "❌ Plan rejected. You can modify your request and try again.".to_string(),
                    prompt,
                );
                self.app.add_block(block);
                self.app.reject_plan();
                return Ok(());
            }
        }

        // Check for slash commands first (e.g., /connect, /help)
        if let Some(slash_cmd) = ShellTuiApp::parse_slash_command(input) {
            return self.execute_slash_command(slash_cmd, ai_tx, orch_tx).await;
        }

        // Check for built-in shell commands (cd, pwd, exit, etc.)
        if ShellTuiApp::is_builtin_command(input) {
            return self.execute_builtin(input);
        }

        // Check if it looks like a shell command
        if ShellTuiApp::looks_like_shell_command(input) {
            return self.execute_shell_command(input, cmd_tx).await;
        }

        // Otherwise, send to AI (with optional @file context)
        self.execute_ai_query(input, ai_tx).await
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
  Shift+Up/Down     Scroll output
  PageUp/PageDown   Scroll output (faster)
  Mouse scroll      Scroll output"#;
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

    /// Execute a slash command (e.g., /connect, /help)
    async fn execute_slash_command(
        &mut self,
        cmd: SlashCommand,
        _ai_tx: mpsc::UnboundedSender<AiUpdate>,
        orch_tx: mpsc::UnboundedSender<OrchestrationUpdate>,
    ) -> Result<()> {
        match cmd {
            SlashCommand::Connect => {
                self.connect_ai().await?;
            }

            SlashCommand::Disconnect => {
                self.disconnect_ai();
            }

            SlashCommand::Agent => {
                self.app.cycle_agent_mode();
                let mode = self.app.agent_mode;
                let prompt = self.app.current_prompt();
                let block = CommandBlock::system(
                    format!("Agent mode: {} - {}", mode.short_name(), mode.description()),
                    prompt,
                );
                self.app.add_block(block);

                // Sync agent mode with session if connected
                if let Some(session) = &self.app.session {
                    let session = session.clone();
                    let agent_mode = mode;
                    tokio::spawn(async move {
                        let mut session = session.lock().await;
                        session.set_agent_mode(agent_mode);
                    });
                }
            }

            SlashCommand::Help => {
                let prompt = self.app.current_prompt();
                let help_text = r#"Safe Coder Shell - AI-Powered Development

Commands:
  /connect          Connect to AI
  /disconnect       Disconnect from AI
  /help             Show this help
  /tools            List available AI tools
  /mode             Toggle permission mode (ASK/EDIT/YOLO)
  /agent            Toggle agent mode (PLAN/BUILD)
  /orchestrate      Run multi-agent task

Shell:
  Type any shell command (ls, git, cargo, etc.)

AI Queries:
  Just type naturally - if it's not a shell command, it goes to AI
  Use @filename to include file contents as context

Examples:
  fix the bug in auth @src/auth.rs
  explain this code @main.rs @lib.rs
  refactor to use async/await @src/**/*.rs

Keyboard:
  Ctrl+C      Cancel/exit
  Ctrl+P      Toggle permission mode
  Ctrl+G      Toggle agent mode
  Ctrl+L      Clear screen
  Tab         Autocomplete"#;
                let block = CommandBlock::system(help_text.to_string(), prompt);
                self.app.add_block(block);
            }

            SlashCommand::Tools => {
                let prompt = self.app.current_prompt();
                let tools_text = r#"Available AI Tools:

  File Operations:
    • read      - Read file contents
    • write     - Write/create files
    • edit      - Edit existing files
    • list      - List directory contents

  Search:
    • glob      - Find files by pattern
    • grep      - Search file contents

  Execution:
    • bash      - Run shell commands

  Web:
    • webfetch  - Fetch URL content

  Task Tracking:
    • todowrite - Update task list
    • todoread  - Read task list"#;
                let block = CommandBlock::system(tools_text.to_string(), prompt);
                self.app.add_block(block);
            }

            SlashCommand::Mode => {
                self.app.cycle_permission_mode();
                let mode = self.app.permission_mode;
                let prompt = self.app.current_prompt();
                let block = CommandBlock::system(
                    format!(
                        "Permission mode: {} - {}",
                        mode.short_name(),
                        mode.description()
                    ),
                    prompt,
                );
                self.app.add_block(block);
            }

            SlashCommand::Commands => {
                self.app.show_commands_modal();
            }

            SlashCommand::Orchestrate(task) => {
                if task.is_empty() {
                    let prompt = self.app.current_prompt();
                    let block = CommandBlock::system(
                        "Usage: /orchestrate <task description>".to_string(),
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

                // Spawn async orchestration execution
                let project_path = self.app.cwd.clone();
                let config = self.config.clone();
                let task_owned = task.clone();
                let orch_tx_clone = orch_tx.clone();
                let block_id_clone = block_id.clone();

                tokio::spawn(async move {
                    let result = std::panic::AssertUnwindSafe(Self::run_orchestration(
                        project_path,
                        config,
                        task_owned,
                        block_id_clone.clone(),
                        orch_tx_clone.clone(),
                    ))
                    .catch_unwind()
                    .await;

                    if let Err(e) = result {
                        let panic_msg = if let Some(s) = e.downcast_ref::<&str>() {
                            s.to_string()
                        } else if let Some(s) = e.downcast_ref::<String>() {
                            s.clone()
                        } else {
                            "Unknown panic".to_string()
                        };
                        let _ = orch_tx_clone.send(OrchestrationUpdate::Error {
                            block_id: block_id_clone,
                            message: format!("Orchestration panicked: {}", panic_msg),
                        });
                    }
                });
            }

            SlashCommand::Models => {
                let prompt = self.app.current_prompt();

                // Get models based on current provider
                let provider = &self.config.llm.provider;
                let output = match provider {
                    crate::config::LlmProvider::GitHubCopilot => {
                        // Try to get models from GitHub Copilot
                        match self.get_copilot_models().await {
                            Ok(models) => {
                                let mut output = String::from("📋 Available GitHub Copilot Models:\n\n");
                                let current_model = &self.config.llm.model;
                                for model in models {
                                    let marker = if model.id == *current_model { " ← current" } else { "" };
                                    let preview = if model.preview.unwrap_or(false) { " (preview)" } else { "" };
                                    output.push_str(&format!("  • {}{}{}\n", model.id, preview, marker));
                                }
                                output.push_str("\nUse /model <name> to switch models.");
                                output
                            }
                            Err(e) => format!("Failed to fetch models: {}\n\nMake sure you're logged in with /login copilot", e),
                        }
                    }
                    crate::config::LlmProvider::Anthropic => {
                        let mut output = String::from("📋 Available Anthropic Models:\n\n");
                        let current_model = &self.config.llm.model;
                        let models = [
                            "claude-opus-4-20250514",
                            "claude-sonnet-4-20250514",
                            "claude-3-5-sonnet-20241022",
                            "claude-3-5-haiku-20241022",
                        ];
                        for model in models {
                            let marker = if model == current_model { " ← current" } else { "" };
                            output.push_str(&format!("  • {}{}\n", model, marker));
                        }
                        output.push_str("\nUse /model <name> to switch models.");
                        output
                    }
                    crate::config::LlmProvider::OpenAI => {
                        let mut output = String::from("📋 Available OpenAI Models:\n\n");
                        let current_model = &self.config.llm.model;
                        let models = ["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "gpt-4"];
                        for model in models {
                            let marker = if model == current_model { " ← current" } else { "" };
                            output.push_str(&format!("  • {}{}\n", model, marker));
                        }
                        output.push_str("\nUse /model <name> to switch models.");
                        output
                    }
                    crate::config::LlmProvider::OpenRouter => {
                        "📋 OpenRouter Models:\n\nVisit https://openrouter.ai/models for the full list.\nUse /model <provider/model-name> to switch.".to_string()
                    }
                    crate::config::LlmProvider::Ollama => {
                        "📋 Ollama Models:\n\nRun `ollama list` to see installed models.\nUse /model <name> to switch.".to_string()
                    }
                };

                let block = CommandBlock::system(output, prompt);
                self.app.add_block(block);
            }

            SlashCommand::Provider(provider_opt) => {
                let prompt = self.app.current_prompt();

                match provider_opt {
                    Some(provider_str) => {
                        // Switch provider
                        let new_provider = match provider_str.to_lowercase().as_str() {
                            "anthropic" | "claude" => Some((crate::config::LlmProvider::Anthropic, "claude-sonnet-4-20250514")),
                            "openai" | "gpt" => Some((crate::config::LlmProvider::OpenAI, "gpt-4o")),
                            "copilot" | "github-copilot" | "github" => Some((crate::config::LlmProvider::GitHubCopilot, "gpt-4o")),
                            "openrouter" => Some((crate::config::LlmProvider::OpenRouter, "anthropic/claude-3.5-sonnet")),
                            "ollama" => Some((crate::config::LlmProvider::Ollama, "llama3")),
                            _ => None,
                        };

                        if let Some((provider, default_model)) = new_provider {
                            self.config.llm.provider = provider.clone();
                            self.config.llm.model = default_model.to_string();

                            // Update display model name in app
                            self.app.model_display = default_model.to_string();

                            // Save config
                            if let Err(e) = self.config.save() {
                                let block = CommandBlock::system(
                                    format!("Warning: Failed to save config: {}", e),
                                    prompt.clone(),
                                );
                                self.app.add_block(block);
                            }

                            let provider_name = match provider {
                                crate::config::LlmProvider::Anthropic => "Anthropic",
                                crate::config::LlmProvider::OpenAI => "OpenAI",
                                crate::config::LlmProvider::GitHubCopilot => "GitHub Copilot",
                                crate::config::LlmProvider::OpenRouter => "OpenRouter",
                                crate::config::LlmProvider::Ollama => "Ollama",
                            };

                            let block = CommandBlock::system(
                                format!("✓ Switched to {} (model: {})\n\nUse /connect to reconnect with new provider.", provider_name, default_model),
                                prompt,
                            );
                            self.app.add_block(block);

                            // Disconnect current AI connection so reconnect uses new provider
                            self.disconnect_ai();
                        } else {
                            let block = CommandBlock::system(
                                format!("Unknown provider: {}\n\nAvailable providers:\n  • anthropic (Claude)\n  • openai (GPT)\n  • copilot (GitHub Copilot)\n  • openrouter\n  • ollama", provider_str),
                                prompt,
                            );
                            self.app.add_block(block);
                        }
                    }
                    None => {
                        // Show current provider
                        let current = match &self.config.llm.provider {
                            crate::config::LlmProvider::Anthropic => "anthropic",
                            crate::config::LlmProvider::OpenAI => "openai",
                            crate::config::LlmProvider::GitHubCopilot => "github-copilot",
                            crate::config::LlmProvider::OpenRouter => "openrouter",
                            crate::config::LlmProvider::Ollama => "ollama",
                        };
                        let block = CommandBlock::system(
                            format!("Current provider: {}\nCurrent model: {}\n\nUse /provider <name> to switch.\nAvailable: anthropic, openai, copilot, openrouter, ollama", current, self.config.llm.model),
                            prompt,
                        );
                        self.app.add_block(block);
                    }
                }
            }

            SlashCommand::Model(model_opt) => {
                let prompt = self.app.current_prompt();

                match model_opt {
                    Some(model_str) => {
                        self.config.llm.model = model_str.clone();

                        // Save config
                        if let Err(e) = self.config.save() {
                            let block = CommandBlock::system(
                                format!("Warning: Failed to save config: {}", e),
                                prompt.clone(),
                            );
                            self.app.add_block(block);
                        }

                        let block = CommandBlock::system(
                            format!("✓ Switched to model: {}\n\nUse /connect to reconnect with new model.", model_str),
                            prompt,
                        );
                        self.app.add_block(block);

                        // Disconnect so reconnect uses new model
                        self.disconnect_ai();
                    }
                    None => {
                        let block = CommandBlock::system(
                            format!("Current model: {}\n\nUse /model <name> to switch.\nUse /models to see available models.", self.config.llm.model),
                            prompt,
                        );
                        self.app.add_block(block);
                    }
                }
            }

            SlashCommand::Login(provider_opt) => {
                let prompt = self.app.current_prompt();
                let provider_str = provider_opt.as_deref().unwrap_or("copilot");

                match provider_str.to_lowercase().as_str() {
                    "copilot" | "github-copilot" | "github" => {
                        let block = CommandBlock::system(
                            "Starting GitHub Copilot login...\n\nPlease follow the device flow in your browser.".to_string(),
                            prompt.clone(),
                        );
                        self.app.add_block(block);

                        // Run login flow
                        match self.login_github_copilot().await {
                            Ok(()) => {
                                // Switch provider to copilot
                                self.config.llm.provider = crate::config::LlmProvider::GitHubCopilot;
                                self.config.llm.model = "gpt-4o".to_string();
                                let _ = self.config.save();

                                let block = CommandBlock::system(
                                    "✓ Successfully logged in to GitHub Copilot!\n\nProvider switched to GitHub Copilot.\nUse /models to see available models.\nUse /connect to connect.".to_string(),
                                    prompt,
                                );
                                self.app.add_block(block);
                            }
                            Err(e) => {
                                let block = CommandBlock::system(
                                    format!("✗ Login failed: {}", e),
                                    prompt,
                                );
                                self.app.add_block(block);
                            }
                        }
                    }
                    "anthropic" | "claude" => {
                        let block = CommandBlock::system(
                            "Anthropic login:\n\nSet your API key in config or environment:\n  export ANTHROPIC_API_KEY=your-key-here\n\nOr edit ~/.config/safe-coder/config.toml".to_string(),
                            prompt,
                        );
                        self.app.add_block(block);
                    }
                    _ => {
                        let block = CommandBlock::system(
                            format!("Unknown provider: {}\n\nAvailable for login:\n  • copilot (GitHub Copilot device flow)\n  • anthropic (API key)", provider_str),
                            prompt,
                        );
                        self.app.add_block(block);
                    }
                }
            }
        }

        Ok(())
    }

    /// Get available models from GitHub Copilot
    async fn get_copilot_models(&self) -> Result<Vec<crate::llm::copilot::CopilotModel>> {
        use crate::config::LlmProvider;

        let token_path = crate::config::Config::token_path(&LlmProvider::GitHubCopilot)?;
        if !token_path.exists() {
            anyhow::bail!("Not logged in to GitHub Copilot");
        }

        let stored_token = crate::auth::StoredToken::load(&token_path)?;
        let github_token = stored_token.get_access_token();

        let copilot_token = crate::llm::copilot::get_copilot_token(github_token).await?;
        let models = crate::llm::copilot::get_copilot_models(&copilot_token).await?;

        Ok(models)
    }

    /// Login to GitHub Copilot using device flow
    async fn login_github_copilot(&mut self) -> Result<()> {
        use crate::auth::github_copilot::GitHubCopilotAuth;
        use crate::auth::{run_device_flow, StoredToken};
        use crate::config::LlmProvider;

        let auth = GitHubCopilotAuth::new();
        let token = run_device_flow(&auth, "GitHub Copilot").await?;

        // Save the token
        let token_path = crate::config::Config::token_path(&LlmProvider::GitHubCopilot)?;
        token.save(&token_path)?;

        Ok(())
    }

    /// Run orchestration asynchronously
    /// This creates a proper plan using UnifiedPlanner before executing tasks
    async fn run_orchestration(
        project_path: PathBuf,
        config: Config,
        task: String,
        block_id: String,
        tx: mpsc::UnboundedSender<OrchestrationUpdate>,
    ) {
        // Send planning status
        let _ = tx.send(OrchestrationUpdate::Planning {
            block_id: block_id.clone(),
        });

        let cli_path = config.orchestrator.claude_cli_path.clone();

        // Create LLM client and use UnifiedPlanner to create a proper plan
        let llm_client = match create_client(&config).await {
            Ok(client) => client,
            Err(e) => {
                let _ = tx.send(OrchestrationUpdate::Complete {
                    block_id,
                    summary: format!("Failed to create LLM client for planning: {}", e),
                    success_count: 0,
                    fail_count: 1,
                });
                return;
            }
        };

        // Use UnifiedPlanner to create a thoughtful, structured plan
        let planner = UnifiedPlanner::new(ExecutionMode::Orchestration);
        let unified_plan = match planner.create_plan(llm_client.as_ref(), &task, None).await {
            Ok(plan) => plan,
            Err(e) => {
                // Fall back to simple single-task plan if planning fails
                eprintln!("Planning failed, using simple plan: {}", e);
                let _ = tx.send(OrchestrationUpdate::TaskOutput {
                    block_id: block_id.clone(),
                    task_id: "planning".to_string(),
                    line: format!(
                        "Note: Advanced planning unavailable ({}), using direct execution",
                        e
                    ),
                });

                // Create fallback simple plan
                use crate::orchestrator::Task;
                let mut plan = TaskPlan::new(
                    "tui-task-1".to_string(),
                    task.clone(),
                    format!("Execute: {}", task),
                );
                plan.add_task(Task::new("task-1".to_string(), task.clone(), task.clone()));

                let _ = tx.send(OrchestrationUpdate::PlanReady {
                    block_id: block_id.clone(),
                    plan: plan.clone(),
                });

                // Execute fallback single task
                Self::execute_single_task(&cli_path, &project_path, &task, block_id, tx).await;
                return;
            }
        };

        // Convert UnifiedPlan to TaskPlan for display
        use crate::orchestrator::Task;
        let mut task_plan = TaskPlan::new(
            unified_plan.id.clone(),
            task.clone(),
            unified_plan.title.clone(),
        );

        // Flatten groups into tasks for execution
        let mut all_tasks: Vec<(String, String, String)> = Vec::new();
        for group in &unified_plan.groups {
            for step in &group.steps {
                task_plan.add_task(Task::new(
                    step.id.clone(),
                    step.description.clone(),
                    step.instructions.clone(),
                ));
                all_tasks.push((
                    step.id.clone(),
                    step.description.clone(),
                    step.instructions.clone(),
                ));
            }
        }

        // Send plan ready
        let _ = tx.send(OrchestrationUpdate::PlanReady {
            block_id: block_id.clone(),
            plan: task_plan.clone(),
        });

        // Send executing status
        let _ = tx.send(OrchestrationUpdate::Executing {
            block_id: block_id.clone(),
            task_count: all_tasks.len(),
        });

        // Execute each task sequentially (groups could be parallelized in full orchestration mode)
        let mut success_count = 0;
        let mut fail_count = 0;
        let mut all_outputs = Vec::new();

        for (task_id, description, instructions) in all_tasks {
            // Send task started
            let _ = tx.send(OrchestrationUpdate::TaskStarted {
                block_id: block_id.clone(),
                task_id: task_id.clone(),
                description: description.clone(),
                worker: "claude".to_string(),
            });

            // Build the full prompt for this step
            let step_prompt = format!(
                "Task: {}\n\nInstructions:\n{}\n\nOriginal request context: {}",
                description, instructions, task
            );

            // Run Claude CLI for this step
            let result = Self::run_claude_cli(
                &cli_path,
                &project_path,
                &step_prompt,
                block_id.clone(),
                task_id.clone(),
                tx.clone(),
            )
            .await;

            let (success, output) = match result {
                Ok(output) => (true, output),
                Err(e) => (false, e.to_string()),
            };

            if success {
                success_count += 1;
            } else {
                fail_count += 1;
            }

            all_outputs.push(format!("## {}\n{}", description, output));

            // Send task completed
            let _ = tx.send(OrchestrationUpdate::TaskCompleted {
                block_id: block_id.clone(),
                task_id: task_id.clone(),
                success,
                output: output.clone(),
            });

            // Stop on failure (could be configurable)
            if !success {
                break;
            }
        }

        // Send completion
        let _ = tx.send(OrchestrationUpdate::Complete {
            block_id,
            summary: if fail_count == 0 {
                format!(
                    "All {} tasks completed successfully:\n\n{}",
                    success_count,
                    all_outputs.join("\n\n")
                )
            } else {
                format!(
                    "{} succeeded, {} failed:\n\n{}",
                    success_count,
                    fail_count,
                    all_outputs.join("\n\n")
                )
            },
            success_count,
            fail_count,
        });
    }

    /// Execute a single task as fallback when planning fails
    async fn execute_single_task(
        cli_path: &str,
        project_path: &PathBuf,
        task: &str,
        block_id: String,
        tx: mpsc::UnboundedSender<OrchestrationUpdate>,
    ) {
        // Send executing status
        let _ = tx.send(OrchestrationUpdate::Executing {
            block_id: block_id.clone(),
            task_count: 1,
        });

        // Send task started
        let _ = tx.send(OrchestrationUpdate::TaskStarted {
            block_id: block_id.clone(),
            task_id: "1".to_string(),
            description: task.to_string(),
            worker: "claude".to_string(),
        });

        // Run Claude CLI directly
        let result = Self::run_claude_cli(
            cli_path,
            project_path,
            task,
            block_id.clone(),
            "1".to_string(),
            tx.clone(),
        )
        .await;

        let (success, output) = match result {
            Ok(output) => (true, output),
            Err(e) => (false, e.to_string()),
        };

        // Send task completed
        let _ = tx.send(OrchestrationUpdate::TaskCompleted {
            block_id: block_id.clone(),
            task_id: "1".to_string(),
            success,
            output: output.clone(),
        });

        // Send completion
        let _ = tx.send(OrchestrationUpdate::Complete {
            block_id,
            summary: if success {
                format!("Task completed successfully:\n{}", output)
            } else {
                format!("Task failed:\n{}", output)
            },
            success_count: if success { 1 } else { 0 },
            fail_count: if success { 0 } else { 1 },
        });
    }

    /// Run Claude CLI directly and return output, streaming to the UI
    async fn run_claude_cli(
        cli_path: &str,
        working_dir: &PathBuf,
        task: &str,
        block_id: String,
        task_id: String,
        tx: mpsc::UnboundedSender<OrchestrationUpdate>,
    ) -> Result<String> {
        use std::process::Stdio;
        use tokio::io::AsyncBufReadExt;

        // Check if claude CLI is available
        let version_check = TokioCommand::new(cli_path).arg("--version").output().await;

        if version_check.is_err() {
            return Err(anyhow::anyhow!(
                "Claude CLI not found at '{}'. Make sure it's installed and in your PATH.",
                cli_path
            ));
        }

        // Run claude with stream-json output for real-time updates
        let mut child = TokioCommand::new(cli_path)
            .current_dir(working_dir)
            .arg("-p")
            .arg(task)
            .arg("--dangerously-skip-permissions")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--verbose")
            .arg("--include-partial-messages")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to spawn Claude CLI")?;

        let stdout = child.stdout.take().context("Failed to capture stdout")?;
        let stderr = child.stderr.take().context("Failed to capture stderr")?;

        // Read stdout and stderr, streaming to UI
        let stdout_reader = TokioBufReader::new(stdout);
        let stderr_reader = TokioBufReader::new(stderr);

        // Clone for the tasks
        let tx_stdout = tx.clone();
        let block_id_stdout = block_id.clone();
        let task_id_stdout = task_id.clone();

        let stdout_task = tokio::spawn(async move {
            let mut final_result = String::new();
            let mut lines = stdout_reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                // Skip empty lines
                if line.trim().is_empty() {
                    continue;
                }

                // Parse JSON events from stream-json output
                let display_line = Self::parse_claude_json_event(&line);

                // Send the parsed line (or indicate we received data)
                if let Some(parsed) = display_line {
                    let _ = tx_stdout.send(OrchestrationUpdate::TaskOutput {
                        block_id: block_id_stdout.clone(),
                        task_id: task_id_stdout.clone(),
                        line: parsed,
                    });
                }

                // Try to extract final result
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                    if json.get("type").and_then(|t| t.as_str()) == Some("result") {
                        if let Some(result) = json.get("result").and_then(|r| r.as_str()) {
                            final_result = result.to_string();
                        }
                    }
                }
            }
            final_result
        });

        let tx_stderr = tx.clone();
        let block_id_stderr = block_id.clone();
        let task_id_stderr = task_id.clone();

        let stderr_task = tokio::spawn(async move {
            let mut errors = String::new();
            let mut lines = stderr_reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                // Send stderr lines to UI with prefix
                let _ = tx_stderr.send(OrchestrationUpdate::TaskOutput {
                    block_id: block_id_stderr.clone(),
                    task_id: task_id_stderr.clone(),
                    line: format!("[stderr] {}", line),
                });
                errors.push_str(&line);
                errors.push('\n');
            }
            errors
        });

        // Wait with timeout (5 minutes)
        let timeout = tokio::time::Duration::from_secs(300);
        let wait_result = tokio::time::timeout(timeout, async {
            let (stdout_result, stderr_result) = tokio::join!(stdout_task, stderr_task);
            let output = stdout_result.unwrap_or_default();
            let errors = stderr_result.unwrap_or_default();
            let status = child.wait().await.context("Failed to wait for process")?;
            Ok::<(String, String, std::process::ExitStatus), anyhow::Error>((
                output, errors, status,
            ))
        })
        .await;

        match wait_result {
            Ok(Ok((output, errors, status))) => {
                if status.success() {
                    Ok(if output.is_empty() {
                        "Task completed".to_string()
                    } else {
                        output
                    })
                } else {
                    Err(anyhow::anyhow!(
                        "Claude CLI exited with status {}: {}",
                        status.code().unwrap_or(-1),
                        errors
                    ))
                }
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err(anyhow::anyhow!(
                "Claude CLI timed out after {} seconds",
                timeout.as_secs()
            )),
        }
    }

    /// Parse a Claude CLI JSON event and return a human-readable display line
    fn parse_claude_json_event(json_line: &str) -> Option<String> {
        let json: serde_json::Value = serde_json::from_str(json_line).ok()?;

        let event_type = json.get("type")?.as_str()?;

        match event_type {
            "system" => {
                let subtype = json.get("subtype").and_then(|s| s.as_str()).unwrap_or("");
                match subtype {
                    "init" => Some("Initializing Claude...".to_string()),
                    _ => None,
                }
            }
            "stream_event" => {
                // Handle streaming events for real-time updates
                if let Some(event) = json.get("event") {
                    let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match event_type {
                        "content_block_start" => {
                            // Tool use or text block starting
                            if let Some(content_block) = event.get("content_block") {
                                let block_type = content_block
                                    .get("type")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or("");
                                if block_type == "tool_use" {
                                    let tool_name = content_block
                                        .get("name")
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("unknown");
                                    let display_name = if tool_name.contains("__") {
                                        tool_name.split("__").last().unwrap_or(tool_name)
                                    } else {
                                        tool_name
                                    };
                                    return Some(format!("▶ Starting tool: {}", display_name));
                                }
                            }
                        }
                        "content_block_delta" => {
                            // Streaming content delta
                            if let Some(delta) = event.get("delta") {
                                let delta_type =
                                    delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                match delta_type {
                                    "text_delta" => {
                                        // Text being streamed - don't show every chunk, too noisy
                                        return None;
                                    }
                                    "input_json_delta" => {
                                        // Tool input being streamed - don't show every chunk
                                        return None;
                                    }
                                    _ => {}
                                }
                            }
                        }
                        "content_block_stop" => {
                            // Block finished
                            return None;
                        }
                        "message_stop" => {
                            return None;
                        }
                        _ => {}
                    }
                }
                None
            }
            "assistant" => {
                // Check for tool use in the message content
                if let Some(message) = json.get("message") {
                    if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
                        for item in content {
                            if let Some(item_type) = item.get("type").and_then(|t| t.as_str()) {
                                match item_type {
                                    "tool_use" => {
                                        let tool_name = item
                                            .get("name")
                                            .and_then(|n| n.as_str())
                                            .unwrap_or("unknown");

                                        // Normalize tool name (strip MCP prefixes)
                                        let display_name = if tool_name.contains("__") {
                                            tool_name.split("__").last().unwrap_or(tool_name)
                                        } else {
                                            tool_name
                                        };

                                        // Try to get a description for common tools
                                        let detail = if tool_name.contains("Bash") {
                                            item.get("input")
                                                .and_then(|i| i.get("command"))
                                                .and_then(|c| c.as_str())
                                                .map(|cmd| {
                                                    // Truncate long commands
                                                    if cmd.len() > 60 {
                                                        format!(": {}...", &cmd[..57])
                                                    } else {
                                                        format!(": {}", cmd)
                                                    }
                                                })
                                                .unwrap_or_default()
                                        } else if tool_name.contains("Read")
                                            || tool_name.contains("Write")
                                            || tool_name.contains("Edit")
                                        {
                                            item.get("input")
                                                .and_then(|i| i.get("file_path").or(i.get("path")))
                                                .and_then(|p| p.as_str())
                                                .map(|path| {
                                                    // Shorten long paths
                                                    if path.len() > 50 {
                                                        format!(": ...{}", &path[path.len() - 47..])
                                                    } else {
                                                        format!(": {}", path)
                                                    }
                                                })
                                                .unwrap_or_default()
                                        } else if tool_name.contains("Glob") {
                                            item.get("input")
                                                .and_then(|i| i.get("pattern"))
                                                .and_then(|p| p.as_str())
                                                .map(|pat| format!(": {}", pat))
                                                .unwrap_or_default()
                                        } else if tool_name.contains("Grep") {
                                            item.get("input")
                                                .and_then(|i| i.get("pattern"))
                                                .and_then(|p| p.as_str())
                                                .map(|pat| format!(": {}", pat))
                                                .unwrap_or_default()
                                        } else {
                                            String::new()
                                        };

                                        return Some(format!(
                                            "Using tool: {}{}",
                                            display_name, detail
                                        ));
                                    }
                                    "text" => {
                                        if let Some(text) =
                                            item.get("text").and_then(|t| t.as_str())
                                        {
                                            // Only show non-empty text that's not too long
                                            let trimmed = text.trim();
                                            if !trimmed.is_empty() {
                                                let display = if trimmed.len() > 100 {
                                                    format!("{}...", &trimmed[..97])
                                                } else {
                                                    trimmed.to_string()
                                                };
                                                return Some(format!("Claude: {}", display));
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                None
            }
            "user" => {
                // Tool results
                if let Some(message) = json.get("message") {
                    if let Some(content) = message.get("content").and_then(|c| c.as_array()) {
                        for item in content {
                            if let Some(item_type) = item.get("type").and_then(|t| t.as_str()) {
                                if item_type == "tool_result" {
                                    let is_error = item
                                        .get("is_error")
                                        .and_then(|e| e.as_bool())
                                        .unwrap_or(false);
                                    if is_error {
                                        return Some("Tool returned error".to_string());
                                    } else {
                                        return Some("Tool completed".to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                None
            }
            "result" => {
                let subtype = json.get("subtype").and_then(|s| s.as_str()).unwrap_or("");
                match subtype {
                    "success" => Some("Task completed successfully".to_string()),
                    "error" => {
                        let error = json
                            .get("error")
                            .and_then(|e| e.as_str())
                            .unwrap_or("Unknown error");
                        Some(format!("Task failed: {}", error))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// Execute an AI query (natural language, possibly with @file context)
    async fn execute_ai_query(
        &mut self,
        input: &str,
        tx: mpsc::UnboundedSender<AiUpdate>,
    ) -> Result<()> {
        if !self.app.ai_connected {
            let prompt = self.app.current_prompt();
            let block =
                CommandBlock::system("AI not connected. Run /connect first.".to_string(), prompt);
            self.app.add_block(block);
            return Ok(());
        }

        // Extract file context from @mentions
        let (query, file_patterns) = ShellTuiApp::extract_file_context(input);

        if query.is_empty() && file_patterns.is_empty() {
            return Ok(());
        }

        // Build file context
        let mut file_context = String::new();
        for pattern in &file_patterns {
            let file_path = self.app.cwd.join(pattern);
            if file_path.exists() && file_path.is_file() {
                match std::fs::read_to_string(&file_path) {
                    Ok(content) => {
                        file_context
                            .push_str(&format!("\n--- File: {} ---\n{}\n", pattern, content));
                    }
                    Err(e) => {
                        file_context
                            .push_str(&format!("\n--- File: {} (error: {}) ---\n", pattern, e));
                    }
                }
            } else {
                // Try as glob pattern
                let full_pattern = self.app.cwd.join(pattern);
                if let Ok(entries) = glob::glob(&full_pattern.to_string_lossy()) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        if entry.is_file() {
                            if let Ok(content) = std::fs::read_to_string(&entry) {
                                let rel_path = entry
                                    .strip_prefix(&self.app.cwd)
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_else(|_| entry.to_string_lossy().to_string());
                                file_context.push_str(&format!(
                                    "\n--- File: {} ---\n{}\n",
                                    rel_path, content
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Create AI query block
        let display_input = if file_patterns.is_empty() {
            input.to_string()
        } else {
            format!("{} (with {} file(s))", query, file_patterns.len())
        };

        let prompt = self.app.current_prompt();
        let block = CommandBlock::new(display_input.clone(), BlockType::AiQuery, prompt);
        let block_id = block.id.clone();
        self.app.add_block(block);
        self.app.set_ai_thinking(true);

        // Update sidebar with current task (use the clean query without @mentions)
        self.app.sidebar.set_task(query.clone());

        // Build full context
        let shell_context = self.app.build_ai_context();
        let full_query = if file_context.is_empty() {
            format!(
                "Context from shell session:\n{}\n\nUser query: {}",
                shell_context, query
            )
        } else {
            format!(
                "Context from shell session:\n{}\n\nFile contents:{}\n\nUser query: {}",
                shell_context, file_context, query
            )
        };

        // Clone session for async task
        if let Some(session) = &self.app.session {
            let session = Arc::clone(session);
            let ai_tx = tx.clone();
            let block_id_clone = block_id.clone();

            // Take attached images (clears them from app state)
            let attached_images: Vec<(String, String)> = self
                .app
                .take_attached_images()
                .into_iter()
                .map(|img| (img.data, img.media_type))
                .collect();

            tokio::spawn(async move {
                let mut session = session.lock().await;

                // Create channel for session events
                let (event_tx, mut event_rx) = mpsc::unbounded_channel::<SessionEvent>();

                // Spawn a task to forward SessionEvents to AiUpdates
                let block_id_inner = block_id_clone.clone();
                let ai_tx_inner = ai_tx.clone();
                let forwarder = tokio::spawn(async move {
                    while let Some(event) = event_rx.recv().await {
                        let update = match event {
                            SessionEvent::Thinking(msg) => AiUpdate::Thinking {
                                block_id: block_id_inner.clone(),
                                message: msg,
                            },
                            SessionEvent::Reasoning(text) => AiUpdate::Reasoning {
                                block_id: block_id_inner.clone(),
                                text,
                            },
                            SessionEvent::ToolStart { name, description } => AiUpdate::ToolStart {
                                block_id: block_id_inner.clone(),
                                tool_name: name,
                                description,
                            },
                            SessionEvent::ToolOutput { name, output } => AiUpdate::ToolOutput {
                                block_id: block_id_inner.clone(),
                                tool_name: name,
                                output,
                            },
                            SessionEvent::BashOutputLine { name, line } => {
                                AiUpdate::BashOutputLine {
                                    block_id: block_id_inner.clone(),
                                    tool_name: name,
                                    line,
                                }
                            }
                            SessionEvent::ToolComplete { name, success } => {
                                AiUpdate::ToolComplete {
                                    block_id: block_id_inner.clone(),
                                    tool_name: name,
                                    success,
                                }
                            }
                            SessionEvent::FileDiff {
                                path,
                                old_content,
                                new_content,
                            } => AiUpdate::FileDiff {
                                block_id: block_id_inner.clone(),
                                path,
                                old_content,
                                new_content,
                            },
                            SessionEvent::TextChunk(text) => AiUpdate::TextChunk {
                                block_id: block_id_inner.clone(),
                                text,
                            },
                            // Subagent events - treat like tool executions
                            SessionEvent::SubagentStarted { id, kind, task } => {
                                AiUpdate::ToolStart {
                                    block_id: block_id_inner.clone(),
                                    tool_name: format!("subagent:{}", id),
                                    description: format!("{} - {}", kind, task),
                                }
                            }
                            SessionEvent::SubagentProgress { id, message } => {
                                AiUpdate::BashOutputLine {
                                    block_id: block_id_inner.clone(),
                                    tool_name: format!("subagent:{}", id),
                                    line: message,
                                }
                            }
                            SessionEvent::SubagentToolUsed {
                                id,
                                tool,
                                description,
                            } => AiUpdate::BashOutputLine {
                                block_id: block_id_inner.clone(),
                                tool_name: format!("subagent:{}", id),
                                line: format!("  {} {}", tool, description),
                            },
                            SessionEvent::SubagentCompleted {
                                id,
                                success,
                                summary,
                            } => AiUpdate::ToolComplete {
                                block_id: block_id_inner.clone(),
                                tool_name: format!("subagent:{}", id),
                                success,
                            },
                            // Plan events - forward for sidebar updates
                            SessionEvent::Plan(plan_event) => AiUpdate::PlanEvent {
                                block_id: block_id_inner.clone(),
                                event: plan_event,
                            },
                            // Plan approval sender - forward to TUI
                            SessionEvent::PlanApprovalSender(tx) => {
                                AiUpdate::PlanApprovalSender(tx)
                            }
                            // Token usage updates
                            SessionEvent::TokenUsage {
                                input_tokens,
                                output_tokens,
                                cache_read_tokens,
                                cache_creation_tokens,
                            } => AiUpdate::TokenUsage {
                                block_id: block_id_inner.clone(),
                                input_tokens,
                                output_tokens,
                                cache_read_tokens,
                                cache_creation_tokens,
                            },
                            // Context compression updates
                            SessionEvent::ContextCompressed { tokens_compressed } => {
                                AiUpdate::ContextCompressed {
                                    block_id: block_id_inner.clone(),
                                    tokens_compressed,
                                }
                            }
                            // Compaction warnings (handled by showing inline warning)
                            SessionEvent::CompactionWarning { message, compaction_count } => {
                                AiUpdate::Reasoning {
                                    block_id: block_id_inner.clone(),
                                    text: format!(
                                        "\n⚠️  **Context Warning** (compaction #{}): {}\n",
                                        compaction_count, message
                                    ),
                                }
                            }
                        };
                        let _ = ai_tx_inner.send(update);
                    }
                });

                // Call the progress-aware method (with images if present)
                match session
                    .send_message_with_images_and_progress(full_query, attached_images, event_tx)
                    .await
                {
                    Ok(response) => {
                        // Wait for forwarder to finish
                        let _ = forwarder.await;

                        let _ = ai_tx.send(AiUpdate::Response {
                            block_id: block_id_clone.clone(),
                            text: response,
                        });
                        let _ = ai_tx.send(AiUpdate::Complete {
                            block_id: block_id_clone,
                        });
                    }
                    Err(e) => {
                        forwarder.abort();
                        let _ = ai_tx.send(AiUpdate::Error {
                            block_id: block_id_clone,
                            message: e.to_string(),
                        });
                    }
                }
            });
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

        // Disable git auto-commit for shell TUI mode to prevent unwanted commits
        let mut config = self.config.clone();
        config.git.auto_commit = false;

        // Debug: log the provider and model being used
        tracing::info!(
            "connect_ai: Using provider {:?}, model {}",
            config.llm.provider,
            config.llm.model
        );

        match Session::new(config, self.app.cwd.clone()).await {
            Ok(session) => {
                self.app.session = Some(Arc::new(Mutex::new(session)));
                self.app.set_ai_connected(true);
                block.complete(
                    "Connected to AI. Just type naturally to ask questions. Use @file to add context.".to_string(),
                    0,
                );
            }
            Err(e) => {
                tracing::error!("connect_ai failed: {:?}", e);
                // Show full error chain for debugging
                let mut error_details = format!("Failed to connect: {}", e);
                let mut source = e.source();
                while let Some(s) = source {
                    error_details.push_str(&format!("\n  Caused by: {}", s));
                    source = s.source();
                }
                block.fail(
                    error_details,
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
