use anyhow::{Context, Result};
use chrono::Utc;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::approval::{ApprovalMode, ExecutionPlan, PlannedTool, UserMode};
use crate::utils::truncate_str;
use crate::checkpoint::{CheckpointManager, DirectoryCheckpointManager};
use crate::config::Config;
use crate::context::ContextManager;
use crate::custom_commands::CustomCommandManager;
use crate::git::GitManager;
use crate::llm::{create_client, ContentBlock, LlmClient, Message, ToolDefinition};
use crate::loop_detector::{DoomLoopAction, LoopDetector};
use crate::lsp::LspManager;
use crate::mcp::McpManager;
use crate::memory::MemoryManager;
use crate::permissions::PermissionManager;
use crate::persistence::{SessionPersistence, SessionStats, ToolUsage};
use crate::unified_planning::create_runner;
use crate::planning::{PlanEvent, PlanStatus, PlanStep, TaskPlan};
use crate::prompts;
use crate::tools::todo::{get_todo_list, increment_turns_without_update, should_show_reminder};
use crate::tools::{AgentMode, ToolContext, ToolRegistry};
// Unified planning imports (reserved for future use)
// use crate::unified_planning::{ExecutionMode as UnifiedExecutionMode, UnifiedPlanner, PlanEvent as UnifiedPlanEvent};
// use crate::unified_planning::integration::create_runner;

/// Maximum characters for tool result content before truncation
/// This prevents context bloat from large tool outputs
const MAX_TOOL_RESULT_CHARS: usize = 8000;

/// Truncate tool result to prevent context bloat
fn truncate_tool_result(result: String) -> String {
    if result.len() <= MAX_TOOL_RESULT_CHARS {
        return result;
    }

    // Find a valid UTF-8 character boundary at or before MAX_TOOL_RESULT_CHARS
    let mut safe_limit = MAX_TOOL_RESULT_CHARS;
    while safe_limit > 0 && !result.is_char_boundary(safe_limit) {
        safe_limit -= 1;
    }

    // Find a good truncation point (newline near the limit)
    let truncate_at = result[..safe_limit]
        .rfind('\n')
        .unwrap_or(safe_limit);

    format!(
        "{}\n\n[Output truncated: {} chars total, showing first {}]",
        &result[..truncate_at],
        result.len(),
        truncate_at
    )
}

/// Events emitted during AI message processing for real-time UI updates
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// AI is thinking/processing (status message)
    Thinking(String),
    /// AI reasoning text before/between tool calls (the LLM's explanation of what it's doing)
    Reasoning(String),
    /// Tool execution started
    ToolStart { name: String, description: String },
    /// Tool produced output
    ToolOutput { name: String, output: String },
    /// Streaming output line from bash command (for inline display)
    BashOutputLine { name: String, line: String },
    /// Tool execution completed
    ToolComplete { name: String, success: bool },
    /// Diagnostic update after file write/edit
    DiagnosticUpdate {
        errors: usize,
        warnings: usize,
    },
    /// File was edited - includes diff info
    FileDiff {
        path: String,
        old_content: String,
        new_content: String,
    },
    /// Text chunk from AI response
    TextChunk(String),
    /// Subagent started
    SubagentStarted {
        id: String,
        kind: String,
        task: String,
    },
    /// Subagent progress update
    SubagentProgress { id: String, message: String },
    /// Subagent is using a tool
    SubagentToolUsed {
        id: String,
        tool: String,
        description: String,
    },
    /// Subagent completed
    SubagentCompleted {
        id: String,
        success: bool,
        summary: String,
    },
    /// Plan event (from planning system)
    Plan(PlanEvent),
    /// Plan approval sender (TUI should store this to send approval)
    /// Note: Using unbounded channel since oneshot::Sender doesn't implement Clone
    PlanApprovalSender(tokio::sync::mpsc::UnboundedSender<bool>),
    /// Token usage update from LLM response
    TokenUsage {
        input_tokens: usize,
        output_tokens: usize,
        /// Tokens read from provider cache (if available)
        cache_read_tokens: Option<usize>,
        /// Tokens written to provider cache (if available)
        cache_creation_tokens: Option<usize>,
    },
    /// Context was compressed - tokens_compressed is the estimated tokens that were compressed
    ContextCompressed { tokens_compressed: usize },
    /// Warning about potential accuracy degradation after multiple compactions
    CompactionWarning {
        message: String,
        compaction_count: usize,
    },
    /// Doom loop detected - asking user whether to continue
    /// The sender is used to receive the user's response (true = continue, false = stop)
    DoomLoopPrompt {
        prompt_id: String,
        message: String,
        response_tx: tokio::sync::mpsc::UnboundedSender<bool>,
    },
    /// Todo list was updated
    TodoList {
        todos: Vec<crate::tools::todo::TodoItem>,
    },
}

pub struct Session {
    config: Config,
    llm_client: Arc<dyn LlmClient>,
    tool_registry: Arc<ToolRegistry>,
    messages: Vec<Message>,
    project_path: PathBuf,

    // Safety & tracking
    git_manager: GitManager,
    loop_detector: LoopDetector,
    context_manager: ContextManager,
    permission_manager: PermissionManager,

    // Features
    persistence: SessionPersistence,
    approval_mode: ApprovalMode,
    user_mode: UserMode,
    agent_mode: AgentMode,
    stats: SessionStats,
    memory: MemoryManager,
    checkpoints: CheckpointManager,
    dir_checkpoints: DirectoryCheckpointManager,
    custom_commands: CustomCommandManager,
    session_start: chrono::DateTime<Utc>,
    current_session_id: Option<String>,
    last_output: String,

    // Event channel for subagent streaming
    subagent_event_tx: Option<mpsc::UnboundedSender<SessionEvent>>,

    // MCP server manager
    mcp_manager: McpManager,

    // LSP manager for diagnostics
    lsp_manager: LspManager,

    // Unified planning state
    current_plan: Option<crate::unified_planning::UnifiedPlan>,
    plan_history: Vec<crate::unified_planning::UnifiedPlan>,
}

impl Session {
    pub async fn new(config: Config, project_path: PathBuf) -> Result<Self> {
        Self::new_with_events(config, project_path, None).await
    }

    /// Create a new session with an optional event channel for subagent streaming
    pub async fn new_with_events(
        config: Config,
        project_path: PathBuf,
        event_tx: Option<mpsc::UnboundedSender<SessionEvent>>,
    ) -> Result<Self> {
        let llm_client: Arc<dyn LlmClient> = Arc::from(create_client(&config).await?);

        // Initialize tool registry with subagent support
        let mut tool_registry = if let Some(tx) = event_tx.clone() {
            ToolRegistry::new()
                .with_subagent_support_and_events(config.clone(), project_path.clone(), tx)
                .await
        } else {
            ToolRegistry::new()
                .with_subagent_support(config.clone(), project_path.clone())
                .await
        };

        // Initialize MCP manager and register its tools
        let mut mcp_manager = McpManager::new(config.mcp.clone());
        mcp_manager.initialize(&project_path).await?;

        // Register MCP tools with the tool registry before wrapping in Arc
        for tool in mcp_manager.get_tools() {
            tool_registry.register(tool);
        }

        let tool_registry = Arc::new(tool_registry);

        if mcp_manager.is_active() {
            tracing::info!(
                "MCP active: {} server(s), {} tool(s)",
                mcp_manager.connected_count(),
                mcp_manager.tool_count()
            );
        }

        // Initialize git for safety
        let git_manager = GitManager::new(project_path.clone());

        // Initialize new features
        let persistence = SessionPersistence::new().await?;
        let memory = MemoryManager::new(project_path.clone());
        let custom_commands = CustomCommandManager::new(project_path.clone()).await?;
        let checkpoints = CheckpointManager::new(project_path.clone());
        let dir_checkpoints =
            DirectoryCheckpointManager::new(project_path.clone(), config.checkpoint.clone())?;

        // Initialize LSP manager for diagnostics
        // Pass None to use default LSP configs - the LspConfigWrapper from config.lsp
        // is for user overrides which we'll apply later if needed
        let mut lsp_manager = LspManager::new(project_path.clone(), None);
        if config.lsp.enabled {
            if let Err(e) = lsp_manager.initialize().await {
                tracing::warn!("LSP initialization failed (continuing without LSP): {}", e);
            }
        }

        // Create context manager with config settings before moving config into struct
        let context_manager = ContextManager::with_config(config.context.to_context_config());

        Ok(Self {
            config,
            llm_client,
            tool_registry,
            messages: vec![],
            project_path: project_path.clone(),

            git_manager,
            loop_detector: LoopDetector::new(),
            context_manager,
            permission_manager: PermissionManager::new(),

            persistence,
            approval_mode: ApprovalMode::default(),
            user_mode: UserMode::default(),
            agent_mode: AgentMode::default(),
            stats: SessionStats::new(),
            memory,
            checkpoints,
            dir_checkpoints,
            custom_commands,
            session_start: Utc::now(),
            current_session_id: None,
            last_output: String::new(),
            subagent_event_tx: event_tx,
            mcp_manager,
            lsp_manager,
            current_plan: None,
            plan_history: Vec::new(),
        })
    }

    /// Set user mode (Plan or Build)
    pub fn set_user_mode(&mut self, mode: UserMode) {
        self.user_mode = mode;
        tracing::info!("User mode set to: {}", mode);
    }

    /// Get current user mode
    pub fn user_mode(&self) -> UserMode {
        self.user_mode
    }

    /// Set agent mode (Plan or Build)
    pub fn set_agent_mode(&mut self, mode: AgentMode) {
        self.agent_mode = mode;
        tracing::info!("Agent mode set to: {}", mode);
    }

    /// Get current agent mode
    pub fn agent_mode(&self) -> AgentMode {
        self.agent_mode
    }

    /// Cycle to next agent mode
    pub fn cycle_agent_mode(&mut self) {
        self.agent_mode = self.agent_mode.next();
        tracing::info!("Agent mode cycled to: {}", self.agent_mode);
    }

    /// Reset the loop detector (used when user chooses to continue after doom loop detection)
    pub fn reset_loop_detector(&mut self) {
        self.loop_detector.reset();
        tracing::info!("Loop detector reset by user");
    }

    /// Apply a permission preset (safe, dev, full, yolo)
    pub fn apply_permission_preset(&mut self, preset: &str) {
        self.permission_manager.apply_preset(preset);
        tracing::info!("Applied permission preset: {}", preset);
    }

    /// Get permission manager summary
    pub fn permission_summary(&self) -> String {
        self.permission_manager.summary()
    }

    /// Get LLM client for unified planning
    pub fn get_llm_client(&self) -> Arc<dyn LlmClient> {
        self.llm_client.clone()
    }

    /// Get tool registry for unified planning
    pub fn get_tool_registry(&self) -> Arc<ToolRegistry> {
        self.tool_registry.clone()
    }

    /// Get the current plan (if any)
    pub fn get_current_plan(&self) -> Option<&crate::unified_planning::UnifiedPlan> {
        self.current_plan.as_ref()
    }

    /// Get plan history
    pub fn get_plan_history(&self) -> &[crate::unified_planning::UnifiedPlan] {
        &self.plan_history
    }

    /// Restore messages from a previous session (for session resumption)
    pub fn restore_messages(&mut self, messages: Vec<Message>) {
        self.messages = messages;
        tracing::info!("Restored {} messages from previous session", self.messages.len());
    }

    /// Get current messages
    pub fn get_messages(&self) -> &[Message] {
        &self.messages
    }

    /// Set the event sender for real-time updates (used by HTTP server)
    pub fn set_event_sender(&mut self, tx: mpsc::UnboundedSender<SessionEvent>) {
        self.subagent_event_tx = Some(tx);
    }

    /// Clear the event sender
    pub fn clear_event_sender(&mut self) {
        self.subagent_event_tx = None;
    }

    /// Send LLM message with automatic retry on token limit errors
    /// Will auto-compact context and retry if token limit is exceeded
    async fn send_llm_with_auto_compact(
        &mut self,
        tools: &[ToolDefinition],
        system_prompt: &str,
        event_tx: Option<&mpsc::UnboundedSender<SessionEvent>>,
    ) -> Result<crate::llm::LlmResponse> {
        const MAX_RETRIES: usize = 3;

        for attempt in 0..MAX_RETRIES {
            match self.llm_client
                .send_message_with_system(&self.messages, tools, Some(system_prompt))
                .await
            {
                Ok(response) => return Ok(response),
                Err(e) => {
                    let error_str = e.to_string().to_lowercase();

                    // Check if it's a token limit error
                    let is_token_limit = error_str.contains("token")
                        && (error_str.contains("exceed")
                            || error_str.contains("limit")
                            || error_str.contains("max_prompt_tokens"));

                    if is_token_limit && attempt < MAX_RETRIES - 1 {
                        // Auto-compact and retry
                        tracing::warn!(
                            "Token limit exceeded (attempt {}), auto-compacting context...",
                            attempt + 1
                        );

                        // Force aggressive compaction
                        let (compacted, result) = self
                            .context_manager
                            .compact(std::mem::take(&mut self.messages));

                        self.messages = compacted;

                        if result.did_compact() {
                            let msg = format!(
                                "Context auto-compacted: {} (attempt {})",
                                result.summary, attempt + 1
                            );
                            tracing::info!("{}", msg);

                            if let Some(tx) = event_tx {
                                let _ = tx.send(SessionEvent::TextChunk(
                                    format!("\n[Context auto-compacted: {}]\n", result.summary)
                                ));
                                let _ = tx.send(SessionEvent::ContextCompressed {
                                    tokens_compressed: result.tokens_saved(),
                                });
                            }
                        } else {
                            // Compaction didn't help, try removing more messages
                            if self.messages.len() > 4 {
                                // Remove older messages (keep system + last 3)
                                let remove_count = self.messages.len() / 2;
                                self.messages.drain(1..=remove_count);

                                if let Some(tx) = event_tx {
                                    let _ = tx.send(SessionEvent::TextChunk(
                                        format!("\n[Removed {} older messages due to token limit]\n", remove_count)
                                    ));
                                }
                            }
                        }

                        // Small delay before retry
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        continue;
                    }

                    // Not a token limit error or max retries reached
                    return Err(e);
                }
            }
        }

        anyhow::bail!("Failed to send message after {} retries due to token limits", MAX_RETRIES)
    }

    /// Set the current plan
    fn set_current_plan(&mut self, plan: crate::unified_planning::UnifiedPlan) {
        // Move any existing plan to history
        if let Some(old_plan) = self.current_plan.take() {
            // Only keep last 10 plans in history
            if self.plan_history.len() >= 10 {
                self.plan_history.remove(0);
            }
            self.plan_history.push(old_plan);
        }
        self.current_plan = Some(plan);
    }

    /// Format current plan for display
    pub fn format_current_plan(&self) -> String {
        match &self.current_plan {
            Some(plan) => {
                let mut output = String::new();
                output.push_str(&format!("## {}\n\n", plan.title));
                output.push_str(&format!("Status: {:?}\n", plan.status));
                output.push_str(&format!("Mode: {:?}\n\n", plan.execution_mode));

                output.push_str("### Steps:\n");
                for (group_idx, group) in plan.groups.iter().enumerate() {
                    if plan.groups.len() > 1 {
                        output.push_str(&format!("\n**Phase {}**", group_idx + 1));
                        if !group.depends_on.is_empty() {
                            output.push_str(&format!(" (depends on: {})", group.depends_on.join(", ")));
                        }
                        output.push('\n');
                    }
                    for step in &group.steps {
                        let status_icon = match step.status {
                            crate::unified_planning::StepStatus::Pending => "◯",
                            crate::unified_planning::StepStatus::InProgress => "◐",
                            crate::unified_planning::StepStatus::Completed => "✓",
                            crate::unified_planning::StepStatus::Failed => "✗",
                            crate::unified_planning::StepStatus::Skipped => "○",
                        };
                        output.push_str(&format!("{} {}\n", status_icon, step.description));
                    }
                }

                if let Some(summary) = plan.groups.iter()
                    .flat_map(|g| g.steps.iter())
                    .filter_map(|s| s.result.as_ref())
                    .last()
                    .map(|r| &r.output)
                {
                    if !summary.is_empty() {
                        output.push_str(&format!("\n### Summary:\n{}\n", summary));
                    }
                }

                output
            }
            None => "No active plan. Submit a task to create a plan.".to_string(),
        }
    }

    /// Format plan groups for display
    pub fn format_plan_groups(&self) -> String {
        match &self.current_plan {
            Some(plan) => {
                let mut output = String::new();
                output.push_str(&format!("## Plan Groups: {}\n\n", plan.title));

                for (i, group) in plan.groups.iter().enumerate() {
                    let status = if group.steps.iter().all(|s| s.status == crate::unified_planning::StepStatus::Completed) {
                        "✓ Completed"
                    } else if group.steps.iter().any(|s| s.status == crate::unified_planning::StepStatus::InProgress) {
                        "◐ In Progress"
                    } else if group.steps.iter().any(|s| s.status == crate::unified_planning::StepStatus::Failed) {
                        "✗ Failed"
                    } else {
                        "◯ Pending"
                    };

                    let parallel_note = if group.steps.len() > 1 {
                        format!(" (parallel: {} steps)", group.steps.len())
                    } else {
                        String::new()
                    };

                    output.push_str(&format!("### Group {}: {}{}\n", i + 1, status, parallel_note));

                    if !group.depends_on.is_empty() {
                        output.push_str(&format!("  Dependencies: {}\n", group.depends_on.join(", ")));
                    }

                    for step in &group.steps {
                        let status_icon = match step.status {
                            crate::unified_planning::StepStatus::Pending => "  ◯",
                            crate::unified_planning::StepStatus::InProgress => "  ◐",
                            crate::unified_planning::StepStatus::Completed => "  ✓",
                            crate::unified_planning::StepStatus::Failed => "  ✗",
                            crate::unified_planning::StepStatus::Skipped => "  ○",
                        };
                        output.push_str(&format!("{} {}\n", status_icon, step.description));
                    }
                    output.push('\n');
                }

                output
            }
            None => "No active plan with groups.".to_string(),
        }
    }

    /// Format plan history for display
    pub fn format_plan_history(&self) -> String {
        if self.plan_history.is_empty() && self.current_plan.is_none() {
            return "No plans executed in this session.".to_string();
        }

        let mut output = String::new();
        output.push_str("## Plan History\n\n");

        // Show current plan first
        if let Some(ref plan) = self.current_plan {
            let status = match plan.status {
                crate::unified_planning::PlanStatus::Completed => "✓ Completed",
                crate::unified_planning::PlanStatus::Failed => "✗ Failed",
                crate::unified_planning::PlanStatus::Executing => "◐ Executing",
                crate::unified_planning::PlanStatus::AwaitingApproval => "⏳ Awaiting Approval",
                _ => "◯ Pending",
            };
            output.push_str(&format!("**Current**: {} [{}] ({:?})\n", plan.title, status, plan.execution_mode));
        }

        // Show history (most recent first)
        for (i, plan) in self.plan_history.iter().rev().enumerate() {
            let status = match plan.status {
                crate::unified_planning::PlanStatus::Completed => "✓",
                crate::unified_planning::PlanStatus::Failed => "✗",
                _ => "○",
            };
            output.push_str(&format!("{}. {} {} ({:?})\n", i + 1, status, plan.title, plan.execution_mode));
        }

        output
    }

    /// Send message using unified planning system
    ///
    /// This uses the new unified planning approach:
    /// 1. Create a plan with the LLM
    /// 2. Show plan (if Plan mode) or execute (if Build mode)
    /// 3. Execute steps with DirectExecutor
    /// 4. Return summary
    pub async fn send_message_with_planning(&mut self, request: String) -> Result<String> {
        use crate::unified_planning::{

            ExecutionMode as UnifiedExecutionMode, UnifiedPlanner,
        };

        // Emit thinking event
        if let Some(ref tx) = self.subagent_event_tx {
            let _ = tx.send(SessionEvent::Thinking(
                "Creating execution plan...".to_string(),
            ));
        }

        // Always use Direct execution for inline shell execution
        let execution_mode = UnifiedExecutionMode::Direct;

        // Create planner
        let planner = UnifiedPlanner::new(execution_mode);

        // Get context
        let project_context = self.memory.get_system_prompt().await.ok();

        // Create plan
        let plan = planner
            .create_plan(&*self.llm_client, &request, project_context.as_deref())
            .await
            .context("Failed to create execution plan")?;

        // Store the plan for /plan commands
        self.set_current_plan(plan.clone());

        // In Plan mode: Create plan, show it, wait for approval, but DON'T execute
        // User must switch to Build mode to execute
        if self.user_mode.requires_approval() {
            use crate::planning::PlanEvent as LegacyEvent;

            // Send plan to UI for display
            if let Some(ref tx) = self.subagent_event_tx {
                let _ = tx.send(SessionEvent::Plan(LegacyEvent::PlanCreated {
                    plan: plan.to_legacy_plan(),
                }));
            }

            // Format plan summary for text response
            let mut plan_text = format!("## Plan: {}\n\n", plan.title);
            plan_text.push_str("**Plan Mode is READ-ONLY. This plan shows what will be done.**\n\n");

            for (i, group) in plan.groups.iter().enumerate() {
                plan_text.push_str(&format!("### Phase {}\n", i + 1));
                for step in &group.steps {
                    plan_text.push_str(&format!("- **{}**: {}\n", step.description, step.instructions));
                    if !step.relevant_files.is_empty() {
                        plan_text.push_str(&format!("  Files: {}\n", step.relevant_files.join(", ")));
                    }
                }
                plan_text.push('\n');
            }

            plan_text.push_str("\n---\n");
            plan_text.push_str("**Plan complete. Switch to BUILD mode (Ctrl+G) to execute this plan.**\n");
            plan_text.push_str("*In Plan mode, Safe-Coder only creates plans - no files are modified.*\n");

            // Send text chunk for display
            if let Some(ref tx) = self.subagent_event_tx {
                let _ = tx.send(SessionEvent::TextChunk(plan_text.clone()));
            }

            return Ok(plan_text);
        }

        // Build mode: Execute the plan immediately
        let runner = create_runner(
            self.project_path.clone(),
            Arc::new(self.config.clone()),
            self.llm_client.clone(),
            self.get_tool_registry(),
        );

        // Execute plan and get events
        let (initial_plan, mut events) = runner.execute(plan).await?;
        let mut final_summary = initial_plan.summary();
        let mut _final_success = false;

        // Forward plan events to session events and await completion
        while let Some(event) = events.recv().await {
            use crate::planning::PlanEvent as LegacyEvent;
            use crate::unified_planning::PlanEvent as UPEvent;

            // Update current plan based on events
            match &event {
                UPEvent::PlanCompleted { summary, success, .. } => {
                    final_summary = summary.clone();
                    _final_success = *success;
                    // Update the stored plan's status
                    if let Some(ref mut plan) = self.current_plan {
                        plan.status = if *success {
                            crate::unified_planning::PlanStatus::Completed
                        } else {
                            crate::unified_planning::PlanStatus::Failed
                        };
                    }
                }
                UPEvent::StepStarted { step_id, .. } => {
                    if let Some(ref mut plan) = self.current_plan {
                        plan.update_step_status(step_id, crate::unified_planning::StepStatus::InProgress);
                    }
                }
                UPEvent::StepCompleted { step_id, success, .. } => {
                    if let Some(ref mut plan) = self.current_plan {
                        plan.update_step_status(
                            step_id,
                            if *success {
                                crate::unified_planning::StepStatus::Completed
                            } else {
                                crate::unified_planning::StepStatus::Failed
                            },
                        );
                    }
                }
                UPEvent::PlanStarted { .. } => {
                    if let Some(ref mut plan) = self.current_plan {
                        plan.status = crate::unified_planning::PlanStatus::Executing;
                    }
                }
                _ => {}
            }

            // Forward to session event stream if available
            if let Some(ref tx) = self.subagent_event_tx {
                // Map common events to legacy format for UI compatibility
                let legacy_event = match &event {
                    UPEvent::PlanCreated { plan, .. } => Some(LegacyEvent::PlanCreated {
                        plan: plan.to_legacy_plan(),
                    }),
                    UPEvent::StepStarted {
                        plan_id,
                        step_id,
                        description,
                        ..
                    } => Some(LegacyEvent::StepStarted {
                        plan_id: plan_id.clone(),
                        step_id: step_id.clone(),
                        description: description.clone(),
                    }),
                    UPEvent::StepProgress {
                        plan_id,
                        step_id,
                        message,
                    } => Some(LegacyEvent::StepProgress {
                        plan_id: plan_id.clone(),
                        step_id: step_id.clone(),
                        message: message.clone(),
                    }),
                    UPEvent::StepCompleted {
                        plan_id,
                        step_id,
                        success,
                        ..
                    } => Some(LegacyEvent::StepCompleted {
                        plan_id: plan_id.clone(),
                        step_id: step_id.clone(),
                        success: *success,
                        output: None,
                        error: None,
                    }),
                    UPEvent::PlanCompleted {
                        plan_id,
                        success,
                        summary,
                    } => Some(LegacyEvent::PlanCompleted {
                        plan_id: plan_id.clone(),
                        success: *success,
                        summary: summary.clone(),
                    }),
                    UPEvent::PlanAwaitingApproval { plan_id } => {
                        Some(LegacyEvent::AwaitingApproval {
                            plan_id: plan_id.clone(),
                        })
                    }
                    UPEvent::PlanApproved { plan_id } => Some(LegacyEvent::PlanApproved {
                        plan_id: plan_id.clone(),
                    }),
                    UPEvent::PlanRejected { plan_id, .. } => Some(LegacyEvent::PlanRejected {
                        plan_id: plan_id.clone(),
                    }),
                    _ => None,
                };

                if let Some(le) = legacy_event {
                    let _ = tx.send(SessionEvent::Plan(le));
                }
            }
        }

        // Return summary
        Ok(final_summary)
    }

    /// Convert unified plan to legacy plan format for events
    fn convert_unified_plan_to_legacy(
        &self,
        plan: &crate::unified_planning::UnifiedPlan,
    ) -> crate::planning::TaskPlan {
        // Use the built-in conversion method
        plan.to_legacy_plan()
    }

    /// Get mutable reference to permission manager for advanced configuration
    pub fn permissions_mut(&mut self) -> &mut PermissionManager {
        &mut self.permission_manager
    }

    pub async fn start(&mut self) -> Result<()> {
        tracing::info!(
            "🔒 Starting Safe Coder session for project: {:?}",
            self.project_path
        );

        // Initialize git if needed and auto-commit is enabled
        if self.config.git.auto_commit {
            self.git_manager.init_if_needed().await?;
            self.git_manager.snapshot("Session start").await?;
            tracing::info!("✓ Session active with git auto-commit enabled");
        } else {
            tracing::info!("✓ Session active (git auto-commit disabled)");
        }

        Ok(())
    }

    /// Coalesce multiple rapid requests into a single LLM call
    /// This is useful when multiple related requests come in quick succession
    pub async fn send_coalesced_messages(&mut self, messages: Vec<String>) -> Result<String> {
        if messages.is_empty() {
            return Ok(String::new());
        }
        
        if messages.len() == 1 {
            return self.send_message(messages.into_iter().next().unwrap()).await;
        }
        
        // Combine multiple messages into a single optimized request
        let combined = format!(
            "Handle these {} related requests efficiently:\n\n{}",
            messages.len(),
            messages
                .iter()
                .enumerate()
                .map(|(i, msg)| format!("{}. {}", i + 1, msg))
                .collect::<Vec<_>>()
                .join("\n\n")
        );
        
        tracing::info!("Coalescing {} requests into single LLM call", messages.len());
        self.send_message(combined).await
    }

    /// Determine if a message should use unified planning
    fn should_use_planning(&self, message: &str) -> bool {
        let msg_lower = message.to_lowercase();
        let msg_len = message.len();
        
        // Skip planning for simple queries
        if msg_lower.starts_with("what") || 
           msg_lower.starts_with("how") || 
           msg_lower.starts_with("explain") || 
           msg_lower.starts_with("why") ||
           msg_lower.starts_with("where") ||
           msg_lower.starts_with("when") ||
           msg_lower.starts_with("who") {
            return false;
        }
        
        // Skip for very short messages (likely simple commands)
        if msg_len < 30 {
            return false;
        }
        
        // Use planning for task-oriented requests
        let has_task_indicators = msg_lower.contains("implement") ||
            msg_lower.contains("create") ||
            msg_lower.contains("build") ||
            msg_lower.contains("add") ||
            msg_lower.contains("modify") ||
            msg_lower.contains("update") ||
            msg_lower.contains("fix") ||
            msg_lower.contains("refactor") ||
            msg_lower.contains("step") ||
            msg_lower.contains("plan") ||
            msg_lower.contains("task") ||
            msg_lower.contains("feature");
            
        // Use planning for longer, complex requests
        has_task_indicators || msg_len > 100
    }

    pub async fn send_message(&mut self, user_message: String) -> Result<String> {
        // Create checkpoint before processing user task (git-agnostic safety)
        if self.dir_checkpoints.is_enabled() {
            let label = user_message.chars().take(100).collect::<String>();
            if let Err(e) = self.dir_checkpoints.create_checkpoint(&label).await {
                tracing::warn!("Failed to create checkpoint: {}", e);
            }
        }

        // Smart fallback: Only try unified planning for appropriate requests
        if self.should_use_planning(&user_message) {
            tracing::debug!("Using unified planning for task-oriented request");

            // Try unified planning path
            // If we have an external event sender (from HTTP server), use it directly
            // Otherwise create an internal channel and drain events
            let has_external_sender = self.subagent_event_tx.is_some();

            let mut internal_rx = if has_external_sender {
                // Use existing external sender - events will be forwarded to HTTP clients
                None
            } else {
                // No external sender, create internal channel (will drain events)
                let (event_tx, event_rx) = mpsc::unbounded_channel();
                self.subagent_event_tx = Some(event_tx);
                Some(event_rx)
            };

            let planning_result = self.send_message_with_planning(user_message.clone()).await;

            // Clean up internal channel if we created one
            if let Some(ref mut rx) = internal_rx {
                while rx.try_recv().is_ok() {}
                self.subagent_event_tx = None;
            }

            if let Ok(resp) = planning_result {
                return Ok(resp);
            }

            tracing::debug!("Planning failed, falling back to direct execution");
        } else {
            tracing::debug!("Using direct execution for simple query");
        }

        // Track stats
        self.stats.total_messages += 1;

        // Add user message to history
        self.messages.push(Message::user(user_message.clone()));

        // Check if context compaction is needed
        if self.context_manager.needs_compaction(&self.messages) {
            let (compacted, result) = self
                .context_manager
                .compact(std::mem::take(&mut self.messages));
            self.messages = compacted;
            if result.did_compact() {
                tracing::info!("Context compacted: {}", result.summary);
            }
        }

        let mut response_text = String::new();

        // Build hierarchical system prompt
        let project_context = self.memory.get_system_prompt().await.ok();
        let system_prompt =
            prompts::build_system_prompt(self.agent_mode, project_context.as_deref(), None);

        loop {
            // Get tools schema
            // Get tools filtered by current agent mode
            let tools: Vec<ToolDefinition> = self
                .tool_registry
                .get_tools_schema_for_mode(self.agent_mode)
                .into_iter()
                .map(|schema| ToolDefinition {
                    name: schema["name"].as_str().unwrap().to_string(),
                    description: schema["description"].as_str().unwrap().to_string(),
                    input_schema: schema["input_schema"].clone(),
                })
                .collect();

            // Send to LLM with hierarchical system prompt, with auto-retry on token limit errors
            tracing::info!("[LLM DEBUG] Sending to LLM, msgs: {}", self.messages.len());
            let llm_start = std::time::Instant::now();
            let llm_response = match self
                .llm_client
                .send_message_with_system(&self.messages, &tools, Some(&system_prompt))
                .await
            {
                Ok(resp) => {
                    tracing::info!("[LLM DEBUG] LLM responded in {:?}", llm_start.elapsed());
                    resp
                }
                Err(e) => {
                    let error_str = e.to_string().to_lowercase();
                    let is_token_limit = error_str.contains("token")
                        && (error_str.contains("exceed")
                            || error_str.contains("limit")
                            || error_str.contains("max_prompt_tokens"));

                    if is_token_limit {
                        // Auto-compact and retry
                        tracing::warn!("Token limit exceeded, auto-compacting context...");
                        let (compacted, result) = self
                            .context_manager
                            .compact(std::mem::take(&mut self.messages));
                        self.messages = compacted;

                        if result.did_compact() {
                            tracing::info!("Context auto-compacted: {}", result.summary);
                        } else if self.messages.len() > 4 {
                            // Remove half of older messages
                            let remove_count = self.messages.len() / 2;
                            self.messages.drain(1..=remove_count);
                            tracing::info!("Removed {} older messages due to token limit", remove_count);
                        }

                        // Retry once after compaction
                        self.llm_client
                            .send_message_with_system(&self.messages, &tools, Some(&system_prompt))
                            .await?
                    } else {
                        return Err(e);
                    }
                }
            };

            let assistant_message = llm_response.message;

            // Track stats from actual token usage if available
            if let Some(usage) = &llm_response.usage {
                self.stats.total_tokens_sent += usage.input_tokens;
                // Record actual tokens for better compaction decisions
                self.context_manager.record_actual_tokens(usage.input_tokens);
                // Check if we need to compact based on actual token usage
                if self.context_manager.needs_compaction_by_actual() {
                    let (compacted, result) = self
                        .context_manager
                        .compact(std::mem::take(&mut self.messages));
                    self.messages = compacted;
                    if result.did_compact() {
                        tracing::info!("Context compacted based on actual tokens: {}", result.summary);
                    }
                }
            } else {
                // Fall back to approximate token counting
                self.stats.total_tokens_sent += user_message.len() / 4;
            }
            self.stats.total_messages += 1;

            // Check if there are any tool calls
            let has_tool_calls = assistant_message
                .content
                .iter()
                .any(|c| matches!(c, ContentBlock::ToolUse { .. }));

            // Extract text from response and emit events
            for block in &assistant_message.content {
                if let ContentBlock::Text { text } = block {
                    // Emit text chunk event for real-time streaming
                    if let Some(ref tx) = self.subagent_event_tx {
                        let _ = tx.send(SessionEvent::TextChunk(text.clone()));
                    }
                    response_text.push_str(text);
                    response_text.push('\n');
                }
            }

            // Add assistant message to history
            self.messages.push(assistant_message.clone());

            if !has_tool_calls {
                // No tool calls, we're done
                break;
            }

            // Build execution plan from tool calls
            let execution_plan = self.build_execution_plan(&assistant_message);

            // Handle based on user mode
            match self.user_mode {
                UserMode::Plan => {
                    // Show detailed plan and ask for approval
                    let plan_output = execution_plan.format_detailed(true);
                    response_text.push_str("\n");
                    response_text.push_str(&plan_output);
                    response_text.push_str("\n");

                    // Check for high-risk operations
                    if execution_plan.has_high_risk() {
                        response_text
                            .push_str("⚠️  WARNING: This plan contains high-risk operations!\n\n");
                    }

                    // Ask for user approval
                    if !self.ask_user_approval().await? {
                        // User declined - add a message asking for alternatives
                        response_text.push_str(
                            "\n❌ Plan rejected. Please provide alternative instructions.\n",
                        );

                        // Remove the assistant message with tool calls
                        self.messages.pop();

                        // Add a user message indicating rejection
                        self.messages.push(Message::user(
                            "The user rejected the execution plan. Please suggest an alternative approach or ask clarifying questions.".to_string()
                        ));

                        // Continue to get alternative from LLM
                        continue;
                    }

                    response_text.push_str("\n✅ Plan approved. Executing...\n\n");
                }
                UserMode::Build => {
                    // Show brief plan summary (not detailed)
                    if !execution_plan.tools.is_empty() {
                        let brief = format!(
                            "🔧 Executing {} tool(s): {}\n",
                            execution_plan.tools.len(),
                            execution_plan
                                .tools
                                .iter()
                                .map(|t| t.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        );
                        response_text.push_str(&brief);
                    }
                }
            }

            // Execute tool calls
            let mut tool_results = Vec::new();
            let mut tools_executed = Vec::new();

            for block in &assistant_message.content {
                if let ContentBlock::ToolUse { id, name, input } = block {
                    // Track stats
                    self.stats.total_tool_calls += 1;

                    // Update tool usage stats
                    let tool_stat = self
                        .stats
                        .tools_used
                        .iter_mut()
                        .find(|t| t.tool_name == *name);

                    if let Some(stat) = tool_stat {
                        stat.count += 1;
                    } else {
                        self.stats.tools_used.push(ToolUsage {
                            tool_name: name.clone(),
                            count: 1,
                        });
                    }

                    tracing::info!("🔧 Executing tool: {}", name);

                    // Check if tool is allowed in current agent mode
                    if !self
                        .tool_registry
                        .can_execute_in_mode(name, self.agent_mode)
                    {
                        let result = format!(
                            "Error: Tool '{}' is not available in {} mode. Switch to BUILD mode to use this tool.",
                            name, self.agent_mode
                        );
                        tool_results.push(ContentBlock::ToolResult {
                            tool_use_id: id.clone(),
                            content: result,
                        });
                        continue;
                    }

                    // Check for doom loop (repeated identical tool calls)
                    match self.loop_detector.check(name, input) {
                        DoomLoopAction::Block { message } => {
                            tracing::warn!("Doom loop blocked: {}", message);
                            tool_results.push(ContentBlock::ToolResult {
                                tool_use_id: id.clone(),
                                content: message,
                            });
                            continue;
                        }
                        DoomLoopAction::Warn { message } => {
                            tracing::warn!("{}", message);
                        }
                        DoomLoopAction::AskUser { message } => {
                            // In non-streaming mode, just warn and continue
                            // (blocking behavior is in send_message_with_progress)
                            tracing::warn!("Doom loop detected: {}", message);
                        }
                        DoomLoopAction::Continue => {}
                    }

                    // Create tool context with working directory and config
                    let tool_ctx = ToolContext::new(&self.project_path, &self.config.tools);

                    let (result, success) = match self.tool_registry.get_tool(name) {
                        Some(tool) => match tool.execute(input.clone(), &tool_ctx).await {
                            Ok(output) => {
                                tools_executed.push(name.clone());
                                // Check if bash command failed (has non-zero exit status)
                                let cmd_success = !output.contains("[Exit status:");
                                (output, cmd_success)
                            }
                            Err(e) => (format!("Error: {}", e), false),
                        },
                        None => (format!("Error: Unknown tool '{}'", name), false),
                    };

                    // Record tool call for doom loop detection
                    self.loop_detector.record(name, input);
                    if success {
                        self.loop_detector.record_success();
                    } else {
                        self.loop_detector.record_failure(&result);
                    }

                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: truncate_tool_result(result),
                    });
                }
            }

            // 🔒 Auto-commit changes after tool execution (if enabled)
            if !tools_executed.is_empty() && self.config.git.auto_commit {
                let commit_message = format!("AI executed: {}", tools_executed.join(", "));
                if let Err(e) = self.git_manager.auto_commit(&commit_message).await {
                    tracing::warn!("Failed to auto-commit changes: {}", e);
                } else {
                    tracing::debug!("✓ Auto-committed: {}", commit_message);
                }
            }

            // Add tool results as a new user message
            if !tool_results.is_empty() {
                let mut final_results = tool_results;

                // Check if any file modifications were made
                let had_file_edits = tools_executed
                    .iter()
                    .any(|t| t == "edit_file" || t == "write_file");

                if had_file_edits {
                    // Give LSP a moment to process file changes
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                    // Run build verification after file modifications
                    if let Some(build_errors) = self.verify_build().await {
                        final_results.push(ContentBlock::Text {
                            text: format!(
                                "\n\n[BUILD ERRORS - FIX NOW]\n{}\n\nYou MUST fix these errors immediately. Do not ask the user - just fix them and verify.",
                                build_errors
                            ),
                        });
                    }

                    // Check for LSP diagnostics after file modifications
                    let diagnostics_summary = self.lsp_manager.get_diagnostics_summary().await;
                    if !diagnostics_summary.is_empty() {
                        final_results.push(ContentBlock::Text {
                            text: format!(
                                "\n\n[LSP ERRORS - FIX NOW]\n{}\n\nFix these issues immediately. Continue working until all errors are resolved.",
                                diagnostics_summary
                            ),
                        });
                    }
                }

                self.messages.push(Message {
                    role: crate::llm::Role::User,
                    content: final_results,
                });
            }
        }

        let final_response = response_text.trim().to_string();
        self.last_output = final_response.clone();

        Ok(final_response)
    }

    /// Send a message with real-time progress updates via channel
    /// This allows the UI to show tool executions as they happen
    pub async fn send_message_with_progress(
        &mut self,
        user_message: String,
        event_tx: mpsc::UnboundedSender<SessionEvent>,
    ) -> Result<String> {
        self.send_message_with_images_and_progress(user_message, vec![], event_tx)
            .await
    }

    /// Send a message with images and real-time progress updates via channel
    /// This allows the UI to show tool executions as they happen
    /// Images are provided as (base64_data, media_type) tuples
    pub async fn send_message_with_images_and_progress(
        &mut self,
        user_message: String,
        images: Vec<(String, String)>,
        event_tx: mpsc::UnboundedSender<SessionEvent>,
    ) -> Result<String> {
        // Create checkpoint before processing user task (git-agnostic safety)
        if self.dir_checkpoints.is_enabled() {
            let label = user_message.chars().take(100).collect::<String>();
            if let Err(e) = self.dir_checkpoints.create_checkpoint(&label).await {
                tracing::warn!("Failed to create checkpoint: {}", e);
            } else {
                let _ = event_tx.send(SessionEvent::TextChunk(
                    "📦 Checkpoint created\n".to_string(),
                ));
            }
        }

        // Track stats
        self.stats.total_messages += 1;

        // Add user message to history (with images if present)
        if images.is_empty() {
            self.messages.push(Message::user(user_message.clone()));
        } else {
            tracing::info!("Sending message with {} image(s)", images.len());
            self.messages
                .push(Message::user_with_images(user_message.clone(), images));
        }

        // Check if context compaction is needed
        if self.context_manager.needs_compaction(&self.messages) {
            let (compacted, result) = self
                .context_manager
                .compact(std::mem::take(&mut self.messages));

            self.messages = compacted;
            if result.did_compact() {
                tracing::info!("Context compacted: {}", result.summary);
                let _ = event_tx.send(SessionEvent::TextChunk(format!(
                    "\n📦 Context compacted: {}\n",
                    result.summary
                )));
                // Send compression event for sidebar token tracking
                let _ = event_tx.send(SessionEvent::ContextCompressed {
                    tokens_compressed: result.tokens_saved(),
                });
            }
        }

        let mut response_text = String::new();

        // Build hierarchical system prompt
        let project_context = self.memory.get_system_prompt().await.ok();
        let system_prompt =
            prompts::build_system_prompt(self.agent_mode, project_context.as_deref(), None);

        // Create a persistent plan ID for this task
        let task_plan_id = format!("plan-{}", uuid::Uuid::new_v4().to_string()[..8].to_string());

        // === PLAN MODE: Create plan, get approval, then execute ===
        if self.agent_mode == AgentMode::Plan || self.user_mode.requires_approval() {
            let _ = event_tx.send(SessionEvent::Thinking("Creating plan...".to_string()));

            // Ask LLM to create a plan (exploration + planning in one call)
            let plan_prompt = format!(
                "The user wants: {}\n\n\
                First, briefly explore the relevant parts of the codebase using the available tools.\n\
                Then create a concise plan showing what steps you would take to accomplish this task.\n\
                Format your plan as:\n\
                ## Plan: [Title]\n\
                1. [Step description]\n\
                2. [Step description]\n\
                ...\n\n\
                Do NOT execute any changes yet - just explore and plan.",
                user_message
            );

            self.messages.push(Message::user(plan_prompt));

            // Let LLM explore and create plan
            let tools: Vec<ToolDefinition> = self
                .tool_registry
                .get_tools_schema_for_mode(AgentMode::Plan) // Read-only tools only
                .into_iter()
                .map(|schema| ToolDefinition {
                    name: schema["name"].as_str().unwrap().to_string(),
                    description: schema["description"].as_str().unwrap().to_string(),
                    input_schema: schema["input_schema"].clone(),
                })
                .collect();

            // Run exploration loop until LLM produces a plan
            loop {
                let llm_response = match self
                    .llm_client
                    .send_message_with_system(&self.messages, &tools, Some(&system_prompt))
                    .await
                {
                    Ok(resp) => resp,
                    Err(e) => {
                        let error_str = e.to_string().to_lowercase();
                        let is_token_limit = error_str.contains("token")
                            && (error_str.contains("exceed")
                                || error_str.contains("limit")
                                || error_str.contains("max_prompt_tokens"));

                        if is_token_limit {
                            // Auto-compact and retry
                            let _ = event_tx.send(SessionEvent::TextChunk(
                                "\n[Auto-compacting context due to token limit...]\n".to_string(),
                            ));
                            let (compacted, result) = self
                                .context_manager
                                .compact(std::mem::take(&mut self.messages));
                            self.messages = compacted;

                            if result.did_compact() {
                                let _ = event_tx.send(SessionEvent::ContextCompressed {
                                    tokens_compressed: result.tokens_saved(),
                                });
                            } else if self.messages.len() > 4 {
                                let remove_count = self.messages.len() / 2;
                                self.messages.drain(1..=remove_count);
                            }

                            self.llm_client
                                .send_message_with_system(&self.messages, &tools, Some(&system_prompt))
                                .await?
                        } else {
                            return Err(e);
                        }
                    }
                };

                let assistant_message = llm_response.message;

                // Check for tool calls
                let has_tool_calls = assistant_message
                    .content
                    .iter()
                    .any(|c| matches!(c, ContentBlock::ToolUse { .. }));

                // Collect reasoning sentences to send interleaved with tool calls
                let mut reasoning_sentences: Vec<String> = Vec::new();
                for block in &assistant_message.content {
                    if let ContentBlock::Text { text } = block {
                        response_text.push_str(text);
                        response_text.push('\n');
                        if has_tool_calls && !text.trim().is_empty() {
                            // Split reasoning by sentences for interleaved display
                            for line in text.split('\n') {
                                if line.trim().is_empty() {
                                    continue;
                                }
                                // Split by sentence boundaries (". " followed by capital letter)
                                let mut current = String::new();
                                let chars: Vec<char> = line.chars().collect();
                                for i in 0..chars.len() {
                                    current.push(chars[i]);
                                    if chars[i] == '.' && i + 2 < chars.len()
                                       && chars[i + 1] == ' '
                                       && chars[i + 2].is_uppercase() {
                                        let sentence = current.trim().to_string();
                                        if !sentence.is_empty() {
                                            reasoning_sentences.push(sentence);
                                        }
                                        current = String::new();
                                    }
                                }
                                let sentence = current.trim().to_string();
                                if !sentence.is_empty() {
                                    reasoning_sentences.push(sentence);
                                }
                            }
                        } else {
                            let _ = event_tx.send(SessionEvent::TextChunk(text.clone()));
                        }
                    }
                }

                self.messages.push(assistant_message.clone());

                if !has_tool_calls {
                    // No more tool calls - plan should be complete
                    break;
                }

                // Execute read-only tool calls
                let mut tool_results = Vec::new();
                let mut step_index = 0usize;
                for block in &assistant_message.content {
                    if let ContentBlock::ToolUse { id, name, input } = block {
                        let description = self.describe_tool_action(name, input);

                        // Send reasoning for this tool (if available)
                        if step_index < reasoning_sentences.len() {
                            let _ = event_tx.send(SessionEvent::Reasoning(
                                reasoning_sentences[step_index].clone()
                            ));
                        }
                        step_index += 1;

                        // Send proper tool events for vertical rendering
                        let _ = event_tx.send(SessionEvent::ToolStart {
                            name: name.clone(),
                            description: description.clone(),
                        });

                        let tool_context = ToolContext::new(&self.project_path, &self.config.tools);
                        let (result, success) = if let Some(tool) = self.tool_registry.get_tool(name) {
                            match tool.execute(input.clone(), &tool_context).await {
                                Ok(r) => {
                                    // Check if bash command failed (has non-zero exit status)
                                    let cmd_success = !r.contains("[Exit status:");
                                    (r, cmd_success)
                                }
                                Err(e) => (format!("Error: {}", e), false),
                            }
                        } else {
                            (format!("Unknown tool: {}", name), false)
                        };

                        // Send tool completion event
                        let _ = event_tx.send(SessionEvent::ToolComplete {
                            name: name.clone(),
                            success,
                        });

                        tool_results.push(ContentBlock::ToolResult {
                            tool_use_id: id.clone(),
                            content: truncate_tool_result(result),
                        });
                    }
                }

                self.messages.push(Message {
                    role: crate::llm::Role::User,
                    content: tool_results,
                });
            }

            // Plan created - parse the plan from response text
            let (plan_title, plan_steps) = parse_plan_from_response(&response_text);

            // Create the TaskPlan for the UI
            let mut task_plan = TaskPlan::new(task_plan_id.clone(), user_message.clone());
            task_plan.title = plan_title;
            task_plan.steps = plan_steps;
            task_plan.status = PlanStatus::AwaitingApproval;

            // Send PlanCreated event FIRST so UI has the plan data
            let _ = event_tx.send(SessionEvent::Plan(PlanEvent::PlanCreated {
                plan: task_plan,
            }));

            // Show approval prompt
            response_text.push_str("\n---\n");
            response_text.push_str("**Plan ready for approval.**\n");
            response_text.push_str("Type `approve` (or `yes`/`y`) to switch to BUILD mode and execute.\n");
            response_text.push_str("Type `reject` (or `no`/`n`) to cancel.\n");
            let _ = event_tx.send(SessionEvent::TextChunk(
                "\n---\n**Plan ready for approval.**\nType `approve` to execute or `reject` to cancel.\n".to_string()
            ));

            // Now send AwaitingApproval to trigger the approval dialog
            let _ = event_tx.send(SessionEvent::Plan(PlanEvent::AwaitingApproval {
                plan_id: task_plan_id.clone(),
            }));

            return Ok(response_text);
        }

        // === BUILD MODE: Direct reactive execution (like Claude Code) ===
        let _ = event_tx.send(SessionEvent::Thinking("Working on it...".to_string()));

        let mut plan_created = false;
        let mut total_step_count = 0usize;

        loop {
            // Notify UI that we're thinking
            let _ = event_tx.send(SessionEvent::Thinking("Processing...".to_string()));

            // Get tools schema
            // Get tools filtered by current agent mode
            let tools: Vec<ToolDefinition> = self
                .tool_registry
                .get_tools_schema_for_mode(self.agent_mode)
                .into_iter()
                .map(|schema| ToolDefinition {
                    name: schema["name"].as_str().unwrap().to_string(),
                    description: schema["description"].as_str().unwrap().to_string(),
                    input_schema: schema["input_schema"].clone(),
                })
                .collect();

            // Send to LLM with hierarchical system prompt, with auto-retry on token limit errors
            tracing::info!("[LLM DEBUG] Sending to LLM, msgs: {}", self.messages.len());
            let llm_start = std::time::Instant::now();
            let llm_response = match self
                .llm_client
                .send_message_with_system(&self.messages, &tools, Some(&system_prompt))
                .await
            {
                Ok(resp) => {
                    tracing::info!("[LLM DEBUG] LLM responded in {:?}", llm_start.elapsed());
                    resp
                }
                Err(e) => {
                    let error_str = e.to_string().to_lowercase();
                    let is_token_limit = error_str.contains("token")
                        && (error_str.contains("exceed")
                            || error_str.contains("limit")
                            || error_str.contains("max_prompt_tokens"));

                    if is_token_limit {
                        // Auto-compact and retry
                        let _ = event_tx.send(SessionEvent::TextChunk(
                            "\n[Auto-compacting context due to token limit...]\n".to_string(),
                        ));
                        let (compacted, result) = self
                            .context_manager
                            .compact(std::mem::take(&mut self.messages));
                        self.messages = compacted;

                        if result.did_compact() {
                            let _ = event_tx.send(SessionEvent::ContextCompressed {
                                tokens_compressed: result.tokens_saved(),
                            });
                        } else if self.messages.len() > 4 {
                            let remove_count = self.messages.len() / 2;
                            self.messages.drain(1..=remove_count);
                        }

                        self.llm_client
                            .send_message_with_system(&self.messages, &tools, Some(&system_prompt))
                            .await?
                    } else {
                        return Err(e);
                    }
                }
            };

            let assistant_message = llm_response.message;

            // Track stats and emit token usage event
            if let Some(usage) = &llm_response.usage {
                self.stats.total_tokens_sent += usage.input_tokens;
                // Record actual tokens for better compaction decisions
                self.context_manager.record_actual_tokens(usage.input_tokens);
                // Emit token usage event for sidebar (including cache stats)
                let _ = event_tx.send(SessionEvent::TokenUsage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    cache_read_tokens: usage.cache_read_tokens,
                    cache_creation_tokens: usage.cache_creation_tokens,
                });
                // Check if we need to compact based on actual token usage
                if self.context_manager.needs_compaction_by_actual() {
                    let (compacted, result) = self
                        .context_manager
                        .compact(std::mem::take(&mut self.messages));
                    self.messages = compacted;
                    if result.did_compact() {
                        tracing::info!("Context compacted based on actual tokens: {}", result.summary);
                        let _ = event_tx.send(SessionEvent::TextChunk(format!(
                            "\n📦 Context compacted (actual tokens exceeded threshold): {}\n",
                            result.summary
                        )));
                    }
                }
            } else {
                // Fall back to approximate token counting
                self.stats.total_tokens_sent += user_message.len() / 4;
            }
            self.stats.total_messages += 1;

            // Check if there are any tool calls
            let has_tool_calls = assistant_message
                .content
                .iter()
                .any(|c| matches!(c, ContentBlock::ToolUse { .. }));

            // Extract text from response
            // Collect reasoning sentences to send interleaved with tool calls
            let mut reasoning_sentences: Vec<String> = Vec::new();
            for block in &assistant_message.content {
                if let ContentBlock::Text { text } = block {
                    response_text.push_str(text);
                    response_text.push('\n');
                    if has_tool_calls && !text.trim().is_empty() {
                        // Split reasoning by sentences for interleaved display
                        // Split on newlines first, then by sentence boundaries
                        for line in text.split('\n') {
                            if line.trim().is_empty() {
                                continue;
                            }
                            // Split by sentence boundaries (". " followed by capital letter)
                            let mut current = String::new();
                            let chars: Vec<char> = line.chars().collect();
                            for i in 0..chars.len() {
                                current.push(chars[i]);
                                // Check for sentence end: ". " followed by capital letter
                                if chars[i] == '.' && i + 2 < chars.len()
                                   && chars[i + 1] == ' '
                                   && chars[i + 2].is_uppercase() {
                                    let sentence = current.trim().to_string();
                                    if !sentence.is_empty() {
                                        reasoning_sentences.push(sentence);
                                    }
                                    current = String::new();
                                }
                            }
                            // Add remaining text as final sentence
                            let sentence = current.trim().to_string();
                            if !sentence.is_empty() {
                                reasoning_sentences.push(sentence);
                            }
                        }
                    } else {
                        let _ = event_tx.send(SessionEvent::TextChunk(text.clone()));
                    }
                }
            }

            // Add assistant message to history
            self.messages.push(assistant_message.clone());

            if !has_tool_calls {
                break;
            }

            // Execute tool calls with progress updates
            let mut tool_results = Vec::new();
            let mut tools_executed = Vec::new();

            // Create a dynamic plan from tool calls for sidebar display
            let tool_calls: Vec<_> = assistant_message
                .content
                .iter()
                .filter_map(|block| {
                    if let ContentBlock::ToolUse { id, name, input } = block {
                        Some((id.clone(), name.clone(), input.clone()))
                    } else {
                        None
                    }
                })
                .collect();

            // Track plan ID for step events - accumulate steps across LLM calls
            let plan_id = if !tool_calls.is_empty() {
                // Create steps from tool calls
                let mut new_steps = Vec::new();
                for (i, (_, name, input)) in tool_calls.iter().enumerate() {
                    let description = self.describe_tool_action(name, input);
                    let step =
                        PlanStep::new(format!("step-{}", total_step_count + i + 1), description);
                    new_steps.push(step);
                }

                if !plan_created {
                    // First time - create the plan with initial steps
                    let mut plan = TaskPlan::new(task_plan_id.clone(), user_message.clone());
                    plan.title = "Executing task".to_string();
                    plan.status = PlanStatus::Executing;
                    plan.steps = new_steps.clone();

                    let _ = event_tx.send(SessionEvent::Plan(PlanEvent::PlanCreated {
                        plan: plan.clone(),
                    }));
                    plan_created = true;
                } else {
                    // Subsequent calls - add steps to existing plan
                    let _ = event_tx.send(SessionEvent::Plan(PlanEvent::StepsAdded {
                        plan_id: task_plan_id.clone(),
                        steps: new_steps.clone(),
                    }));
                }

                Some(task_plan_id.clone())
            } else {
                None
            };

            // Track the step offset for this iteration (before incrementing total_step_count)
            let step_offset = total_step_count;
            // Update total_step_count with number of tools in this batch
            total_step_count += tool_calls.len();

            let mut step_index = 0usize;
            for block in &assistant_message.content {
                if let ContentBlock::ToolUse { id, name, input } = block {
                    // Track stats
                    self.stats.total_tool_calls += 1;

                    // Update tool usage stats
                    let tool_stat = self
                        .stats
                        .tools_used
                        .iter_mut()
                        .find(|t| t.tool_name == *name);

                    if let Some(stat) = tool_stat {
                        stat.count += 1;
                    } else {
                        self.stats.tools_used.push(ToolUsage {
                            tool_name: name.clone(),
                            count: 1,
                        });
                    }

                    // Check if tool is allowed in current agent mode
                    if !self
                        .tool_registry
                        .can_execute_in_mode(name, self.agent_mode)
                    {
                        let result = format!(
                            "Error: Tool '{}' is not available in {} mode. Switch to BUILD mode to use this tool.",
                            name, self.agent_mode
                        );
                        let _ = event_tx.send(SessionEvent::ToolStart {
                            name: name.clone(),
                            description: format!("Blocked: {}", name),
                        });
                        let _ = event_tx.send(SessionEvent::ToolOutput {
                            name: name.clone(),
                            output: result.clone(),
                        });
                        let _ = event_tx.send(SessionEvent::ToolComplete {
                            name: name.clone(),
                            success: false,
                        });
                        tool_results.push(ContentBlock::ToolResult {
                            tool_use_id: id.clone(),
                            content: result,
                        });
                        continue;
                    }

                    // Check for doom loop (repeated identical tool calls)
                    match self.loop_detector.check(name, input) {
                        DoomLoopAction::Block { message } => {
                            let _ = event_tx.send(SessionEvent::ToolStart {
                                name: name.clone(),
                                description: format!("Blocked (doom loop): {}", name),
                            });
                            let _ = event_tx.send(SessionEvent::ToolOutput {
                                name: name.clone(),
                                output: message.clone(),
                            });
                            let _ = event_tx.send(SessionEvent::ToolComplete {
                                name: name.clone(),
                                success: false,
                            });
                            tool_results.push(ContentBlock::ToolResult {
                                tool_use_id: id.clone(),
                                content: message,
                            });
                            continue;
                        }
                        DoomLoopAction::Warn { message } => {
                            // Log warning but continue
                            tracing::warn!("{}", message);
                            let _ =
                                event_tx.send(SessionEvent::TextChunk(format!("\n{}\n", message)));
                        }
                        DoomLoopAction::AskUser { message } => {
                            tracing::warn!("Doom loop detected, asking user: {}", message);

                            // Create channel for user response
                            let (response_tx, mut response_rx) = tokio::sync::mpsc::unbounded_channel::<bool>();
                            let prompt_id = uuid::Uuid::new_v4().to_string();

                            // Send prompt to UI
                            let _ = event_tx.send(SessionEvent::DoomLoopPrompt {
                                prompt_id: prompt_id.clone(),
                                message: message.clone(),
                                response_tx,
                            });

                            // Wait for user response (with timeout)
                            let should_continue = tokio::time::timeout(
                                std::time::Duration::from_secs(300), // 5 minute timeout
                                response_rx.recv()
                            ).await.ok().flatten().unwrap_or(false);

                            if !should_continue {
                                tracing::info!("User chose to stop due to doom loop (prompt_id={})", prompt_id);
                                tool_results.push(ContentBlock::ToolResult {
                                    tool_use_id: id.clone(),
                                    content: "Operation stopped by user due to detected loop pattern.".to_string(),
                                });
                                continue;
                            }

                            // User chose to continue - reset loop detector
                            self.loop_detector.reset();
                            tracing::info!("User chose to continue past doom loop (prompt_id={})", prompt_id);
                        }
                        DoomLoopAction::Continue => {}
                    }

                    // Generate description for the tool action
                    let description = self.describe_tool_action(name, input);

                    // Send reasoning for this tool (if available)
                    // Each tool gets one reasoning sentence for interleaved display
                    if step_index < reasoning_sentences.len() {
                        let _ = event_tx.send(SessionEvent::Reasoning(
                            reasoning_sentences[step_index].clone()
                        ));
                    }

                    // Notify UI that tool is starting
                    let _ = event_tx.send(SessionEvent::ToolStart {
                        name: name.clone(),
                        description: description.clone(),
                    });

                    // Emit step started event for plan sidebar
                    if let Some(ref pid) = plan_id {
                        let step_id = format!("step-{}", step_offset + step_index + 1);
                        let _ = event_tx.send(SessionEvent::Plan(PlanEvent::StepStarted {
                            plan_id: pid.clone(),
                            step_id: step_id.clone(),
                            description: description.clone(),
                        }));
                    }

                    // For edit_file, capture old content for diff
                    // Note: edit_file uses "file_path", write_file uses "path"
                    let old_content = if name == "edit_file" {
                        if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                            let full_path = self.project_path.join(path);
                            std::fs::read_to_string(&full_path).ok()
                        } else {
                            None
                        }
                    } else if name == "write_file" {
                        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                            let full_path = self.project_path.join(path);
                            std::fs::read_to_string(&full_path).ok()
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    // Create tool context - use streaming callback for bash commands
                    // Also pass session event channel for subagent streaming
                    let tool_ctx = if name == "bash" {
                        // Create a streaming callback for bash output
                        let event_tx_clone = event_tx.clone();
                        let tool_name = name.clone();
                        let callback: crate::tools::OutputCallback =
                            Arc::new(move |line: String| {
                                let _ = event_tx_clone.send(SessionEvent::BashOutputLine {
                                    name: tool_name.clone(),
                                    line,
                                });
                            });
                        ToolContext::with_output_callback(
                            &self.project_path,
                            &self.config.tools,
                            callback,
                        )
                        .with_session_events(event_tx.clone())
                    } else {
                        ToolContext::new(&self.project_path, &self.config.tools)
                            .with_session_events(event_tx.clone())
                    };

                    tracing::info!("[TOOL DEBUG] Starting tool execution: {}", name);
                    let tool_start = std::time::Instant::now();

                    let (result, success) = match self.tool_registry.get_tool(name) {
                        Some(tool) => match tool.execute(input.clone(), &tool_ctx).await {
                            Ok(output) => {
                                tracing::info!("[TOOL DEBUG] Tool {} completed OK in {:?}, output len: {}",
                                    name, tool_start.elapsed(), output.len());
                                tools_executed.push(name.clone());
                                // Check if bash command failed (has non-zero exit status)
                                let cmd_success = !output.contains("[Exit status:");
                                (output, cmd_success)
                            }
                            Err(e) => {
                                tracing::error!("[TOOL DEBUG] Tool {} failed in {:?}: {}",
                                    name, tool_start.elapsed(), e);
                                (format!("Error: {}", e), false)
                            },
                        },
                        None => {
                            tracing::error!("[TOOL DEBUG] Unknown tool: {}", name);
                            (format!("Error: Unknown tool '{}'", name), false)
                        },
                    };
                    tracing::info!("[TOOL DEBUG] Tool {} finished, success: {}", name, success);

                    // Record tool call for doom loop detection
                    self.loop_detector.record(name, input);
                    if success {
                        self.loop_detector.record_success();
                    } else {
                        self.loop_detector.record_failure(&result);
                        // Check for failure loop
                        if let Some(DoomLoopAction::AskUser { message }) =
                            self.loop_detector.check_failure_loop()
                        {
                            tracing::warn!("Failure loop detected, asking user: {}", message);

                            // Create channel for user response
                            let (response_tx, mut response_rx) = tokio::sync::mpsc::unbounded_channel::<bool>();
                            let prompt_id = uuid::Uuid::new_v4().to_string();

                            // Send prompt to UI
                            let _ = event_tx.send(SessionEvent::DoomLoopPrompt {
                                prompt_id: prompt_id.clone(),
                                message: message.clone(),
                                response_tx,
                            });

                            // Wait for user response (with timeout)
                            let should_continue = tokio::time::timeout(
                                std::time::Duration::from_secs(300),
                                response_rx.recv()
                            ).await.ok().flatten().unwrap_or(false);

                            if !should_continue {
                                tracing::info!("User chose to stop due to failure loop (prompt_id={})", prompt_id);
                                // Break out of the loop - session will end
                                break;
                            }

                            // User chose to continue - reset loop detector
                            self.loop_detector.reset();
                            tracing::info!("User chose to continue past failure loop (prompt_id={})", prompt_id);
                        }
                    }

                    // For edit_file/write_file, send diff event for sidebar
                    // Note: edit_file uses "file_path", write_file uses "path"
                    if (name == "edit_file" || name == "write_file") && success {
                        let path_key = if name == "edit_file" {
                            "file_path"
                        } else {
                            "path"
                        };
                        if let Some(path) = input.get(path_key).and_then(|v| v.as_str()) {
                            let full_path = self.project_path.join(path);
                            tracing::info!("Sending FileDiff event for: {}", path);

                            // Send diff event - use empty string for old_content if file is new
                            if let Ok(new_content) = std::fs::read_to_string(&full_path) {
                                tracing::info!("FileDiff: old_len={}, new_len={}", old_content.as_ref().map(|s| s.len()).unwrap_or(0), new_content.len());
                                let _ = event_tx.send(SessionEvent::FileDiff {
                                    path: path.to_string(),
                                    old_content: old_content.unwrap_or_default(),
                                    new_content: new_content.clone(),
                                });
                            } else {
                                tracing::warn!("Could not read file for FileDiff: {}", full_path.display());
                            }

                            // Notify LSP of file change for diagnostics
                            if let Err(e) = self.lsp_manager.notify_file_changed(&full_path).await {
                                tracing::debug!("LSP file change notification failed: {}", e);
                            }

                            // Give LSP a moment to process and send diagnostic update
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            let (errors, warnings) = self.lsp_manager.get_diagnostic_counts().await;
                            let _ = event_tx.send(SessionEvent::DiagnosticUpdate { errors, warnings });
                        }
                    }

                    // For todowrite, send todo list update event
                    if name == "todowrite" && success {
                        let todos = crate::tools::todo::get_todo_list();
                        let _ = event_tx.send(SessionEvent::TodoList { todos });
                    }

                    // Send tool output
                    let _ = event_tx.send(SessionEvent::ToolOutput {
                        name: name.clone(),
                        output: result.clone(),
                    });

                    // Notify UI that tool completed
                    let _ = event_tx.send(SessionEvent::ToolComplete {
                        name: name.clone(),
                        success,
                    });

                    // Emit step completed event for plan sidebar
                    if let Some(ref pid) = plan_id {
                        let step_id = format!("step-{}", step_offset + step_index + 1);
                        let _ = event_tx.send(SessionEvent::Plan(PlanEvent::StepCompleted {
                            plan_id: pid.clone(),
                            step_id,
                            success,
                            output: if success { Some(result.clone()) } else { None },
                            error: if !success { Some(result.clone()) } else { None },
                        }));
                    }
                    step_index += 1;

                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: truncate_tool_result(result),
                    });
                }
            }

            // Emit plan completed event
            if let Some(ref pid) = plan_id {
                let all_success = tool_results.iter().all(|r| {
                    if let ContentBlock::ToolResult { content, .. } = r {
                        !content.starts_with("Error:")
                    } else {
                        true
                    }
                });
                let _ = event_tx.send(SessionEvent::Plan(PlanEvent::PlanCompleted {
                    plan_id: pid.clone(),
                    success: all_success,
                    summary: format!("Completed {} tool(s)", tools_executed.len()),
                }));
            }

            // Auto-commit if enabled
            if !tools_executed.is_empty() && self.config.git.auto_commit {
                let commit_message = format!("AI executed: {}", tools_executed.join(", "));
                if let Err(e) = self.git_manager.auto_commit(&commit_message).await {
                    tracing::warn!("Failed to auto-commit changes: {}", e);
                }
            }

            // Add tool results as a new user message
            if !tool_results.is_empty() {
                let mut final_results = tool_results;
                let mut has_issues = false;

                // Check if any file modifications were made
                let had_file_edits = tools_executed
                    .iter()
                    .any(|t| t == "edit_file" || t == "write_file");

                if had_file_edits {
                    // Give LSP a moment to process file changes
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

                    // Run build verification after file modifications
                    if let Some(build_errors) = self.verify_build().await {
                        has_issues = true;

                        // Track error patterns to detect loops
                        if let Some(loop_action) = self.loop_detector.record_build_errors(&build_errors) {
                            // Same error type keeps occurring - add stronger hint to LLM
                            match loop_action {
                                DoomLoopAction::AskUser { message: loop_msg } => {
                                    // Log the loop detection
                                    let _ = event_tx.send(SessionEvent::TextChunk(
                                        format!("\n⚠️ {}\n", loop_msg),
                                    ));
                                    // Add hint to the LLM to try a different approach
                                    final_results.push(ContentBlock::Text {
                                        text: format!(
                                            "\n\n[BUILD ERRORS - SAME ERROR REPEATING]\n{}\n\n\
                                            ⚠️ This same error has occurred multiple times. Your previous fixes did not work.\n\
                                            STOP and THINK: What is the ROOT CAUSE? Try a completely different approach.\n\
                                            Do NOT repeat the same fix.",
                                            build_errors
                                        ),
                                    });
                                }
                                _ => {
                                    final_results.push(ContentBlock::Text {
                                        text: format!(
                                            "\n\n[BUILD ERRORS - FIX NOW]\n{}\n\nYou MUST fix these errors immediately. Do not ask the user - just fix them and verify.",
                                            build_errors
                                        ),
                                    });
                                }
                            }
                        } else {
                            final_results.push(ContentBlock::Text {
                                text: format!(
                                    "\n\n[BUILD ERRORS - FIX NOW]\n{}\n\nYou MUST fix these errors immediately. Do not ask the user - just fix them and verify.",
                                    build_errors
                                ),
                            });
                        }

                        let _ = event_tx.send(SessionEvent::TextChunk(
                            "\n⚙️ Build errors detected - fixing...\n".to_string(),
                        ));
                    } else {
                        // Build succeeded - clear error tracking
                        self.loop_detector.clear_error_tracking();
                    }

                    // Check for LSP diagnostics after file modifications
                    let diagnostics_summary = self.lsp_manager.get_diagnostics_summary().await;
                    if !diagnostics_summary.is_empty() {
                        has_issues = true;
                        final_results.push(ContentBlock::Text {
                            text: format!(
                                "\n\n[LSP ERRORS - FIX NOW]\n{}\n\nFix these issues immediately. Continue working until all errors are resolved.",
                                diagnostics_summary
                            ),
                        });

                        let _ = event_tx.send(SessionEvent::TextChunk(
                            "\n⚙️ LSP issues detected - fixing...\n".to_string(),
                        ));
                    }

                    // If no issues, confirm build success
                    if !has_issues {
                        let _ = event_tx.send(SessionEvent::TextChunk(
                            "\n✓ Build verified successfully\n".to_string(),
                        ));
                    }
                }

                self.messages.push(Message {
                    role: crate::llm::Role::User,
                    content: final_results,
                });
            }
        }

        // Increment turns without todo update counter
        increment_turns_without_update();

        // Check if we should show a soft reminder about todo updates
        if should_show_reminder() {
            let todos = get_todo_list();
            if !todos.is_empty() {
                // Only remind if there are actual todos to update
                let in_progress_count = todos.iter().filter(|t| t.status == "in_progress").count();
                let pending_count = todos.iter().filter(|t| t.status == "pending").count();
                if in_progress_count > 0 || pending_count > 0 {
                    let reminder = format!(
                        "\n📋 Reminder: You have {} in-progress and {} pending tasks. Consider updating your todo list.",
                        in_progress_count, pending_count
                    );
                    let _ = event_tx.send(SessionEvent::TextChunk(reminder));
                }
            }
        }

        let final_response = response_text.trim().to_string();
        self.last_output = final_response.clone();

        Ok(final_response)
    }

    /// Build an execution plan from an assistant message with tool calls
    fn build_execution_plan(&self, assistant_message: &Message) -> ExecutionPlan {
        let mut plan = ExecutionPlan::new();

        // Extract summary from text content
        for block in &assistant_message.content {
            if let ContentBlock::Text { text } = block {
                // Use first sentence as summary
                if let Some(first_sentence) = text.split('.').next() {
                    if plan.summary.is_empty() && first_sentence.len() < 200 {
                        plan.summary = first_sentence.trim().to_string();
                    }
                }
            }
        }

        // Add tools to plan
        for block in &assistant_message.content {
            if let ContentBlock::ToolUse { name, input, .. } = block {
                let description = self.describe_tool_action(name, input);
                let planned_tool = PlannedTool::new(name.clone(), description)
                    .with_params(input.clone())
                    .auto_risk();
                plan.add_tool(planned_tool);
            }
        }

        // Estimate complexity based on number of tools and risk levels
        let complexity = match plan.tools.len() {
            0 => 1,
            1 => 2,
            2..=3 => 3,
            4..=5 => 4,
            _ => 5,
        };
        plan.set_complexity(complexity);

        // Add risks for high-risk operations
        let high_risk_tools: Vec<_> = plan
            .tools
            .iter()
            .filter(|t| t.risk_level == crate::approval::RiskLevel::High)
            .map(|t| format!("High-risk operation: {} - {}", t.name, t.description))
            .collect();
        for risk in high_risk_tools {
            plan.add_risk(risk);
        }

        plan
    }

    /// Get the build command for this project from config
    pub fn get_build_command(&self) -> Option<String> {
        self.config.build.get_build_command(&self.project_path)
    }

    /// Get build command hint for prompts
    pub fn get_build_command_hint(&self) -> String {
        self.config.build.get_build_command_hint(&self.project_path)
    }

    /// Run build verification and return any errors
    ///
    /// This runs the project's build command and returns the output if there are errors.
    /// Returns None if build succeeds or if no build command is available.
    pub async fn verify_build(&self) -> Option<String> {
        let build_cmd = self.get_build_command()?;
        let timeout = self.config.build.timeout_secs;
        let max_output = self.config.build.max_output_bytes;

        // Run the build command with timeout
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout),
            tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&build_cmd)
                .current_dir(&self.project_path)
                .output(),
        )
        .await;

        let output = match output {
            Ok(Ok(o)) => o,
            Ok(Err(_)) => return Some("Build command failed to execute".to_string()),
            Err(_) => return Some(format!("Build timed out after {}s", timeout)),
        };

        if output.status.success() {
            None
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let combined = format!("{}{}", stdout, stderr);

            // Filter to only include actual errors, not warnings
            // For Rust: look for "error[" or "error:" patterns
            // For TypeScript/JS: look for "error TS" or "Error:"
            let error_lines: Vec<&str> = combined
                .lines()
                .filter(|line| {
                    let l = line.to_lowercase();
                    // Include lines with actual errors
                    l.contains("error[") ||      // Rust error codes
                    l.contains("error:") ||       // Generic errors
                    l.contains("error ts") ||     // TypeScript
                    l.contains("cannot find") ||  // Common error pattern
                    l.contains("not found") ||    // Common error pattern
                    l.contains("failed to") ||    // Build failures
                    l.contains("aborting due")    // Rust abort message
                })
                .take(30) // Limit to 30 error lines
                .collect();

            // Only return if there are actual errors
            if error_lines.is_empty() {
                // Build failed but no error lines found - might be a different issue
                None // Don't report warnings-only failures
            } else {
                let error_output = error_lines.join("\n");
                // Limit output to configured max size
                let truncated = if error_output.len() > max_output {
                    format!("{}...\n[output truncated]", &error_output[..max_output])
                } else {
                    error_output
                };
                Some(truncated)
            }
        }
    }

    /// Generate a compact description of a tool action (no emojis)
    fn describe_tool_action(&self, name: &str, params: &serde_json::Value) -> String {
        match name {
            "read_file" => {
                let path = params.get("file_path")
                    .or_else(|| params.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                path.to_string()
            }
            "write_file" => {
                let path = params.get("file_path")
                    .or_else(|| params.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                path.to_string()
            }
            "edit_file" => {
                let path = params.get("file_path")
                    .or_else(|| params.get("path"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                path.to_string()
            }
            "glob" => {
                let pattern = params.get("pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                pattern.to_string()
            }
            "grep" | "code_search" => {
                let pattern = params.get("pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                format!("\"{}\"", pattern)
            }
            "list" => {
                let path = params.get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or(".");
                path.to_string()
            }
            "bash" => {
                let cmd = params.get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                if cmd.chars().count() > 40 {
                    format!("{}...", truncate_str(cmd, 37))
                } else {
                    cmd.to_string()
                }
            }
            "webfetch" => {
                let url = params.get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                if url.chars().count() > 40 {
                    format!("{}...", truncate_str(url, 37))
                } else {
                    url.to_string()
                }
            }
            "todowrite" => {
                let count = params.get("todos")
                    .and_then(|v| v.as_array())
                    .map(|a| a.len())
                    .unwrap_or(0);
                format!("{} items", count)
            }
            _ => name.to_string(),
        }
    }

    /// Ask user for approval (for Plan mode)
    async fn ask_user_approval(&self) -> Result<bool> {
        print!("\n🔒 Execute this plan? [y/N]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim().to_lowercase();
        Ok(input == "y" || input == "yes")
    }

    pub async fn stop(&mut self) -> Result<()> {
        tracing::info!("Ending Safe Coder session");

        // Show final change summary if git tracking is enabled
        if self.config.git.auto_commit {
            if let Ok(summary) = self.git_manager.get_change_summary().await {
                if summary.has_changes() {
                    tracing::info!("Session changes:\n{}", summary.summary_text());
                }
            }
        }

        Ok(())
    }

    // ========== New Command Support Methods ==========

    /// Get project directory for at-commands and shell execution
    pub fn get_sandbox_dir(&self) -> Result<PathBuf> {
        Ok(self.project_path.clone())
    }

    /// Execute shell command in project directory
    pub async fn execute_shell_command(&self, command: &str) -> Result<String> {
        // Use bash tool to execute command
        let bash_tool = self
            .tool_registry
            .get_tool("bash")
            .context("Bash tool not found")?;

        let input = serde_json::json!({
            "command": command
        });

        let tool_ctx = ToolContext::new(&self.project_path, &self.config.tools);
        bash_tool.execute(input, &tool_ctx).await
    }

    /// Execute shell command in project directory with streaming output
    pub async fn execute_shell_command_streaming<F>(
        &self,
        command: &str,
        output_callback: F,
    ) -> Result<String>
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        use crate::tools::OutputCallback;
        use std::sync::Arc;

        // Use bash tool to execute command
        let bash_tool = self
            .tool_registry
            .get_tool("bash")
            .context("Bash tool not found")?;

        let input = serde_json::json!({
            "command": command
        });

        let callback: OutputCallback = Arc::new(output_callback);
        let tool_ctx =
            ToolContext::with_output_callback(&self.project_path, &self.config.tools, callback);
        bash_tool.execute(input, &tool_ctx).await
    }

    /// Get session statistics
    pub async fn get_stats(&mut self) -> Result<String> {
        // Update session duration
        let duration = Utc::now() - self.session_start;
        self.stats.session_duration_secs = duration.num_seconds();

        Ok(self.stats.format())
    }

    /// Save current chat session
    pub async fn save_chat(&mut self, name: Option<String>) -> Result<String> {
        let id = self
            .persistence
            .save_session(name, &self.project_path, &self.messages)
            .await?;

        self.current_session_id = Some(id.clone());
        Ok(id)
    }

    /// Resume a saved chat session
    pub async fn resume_chat(&mut self, id: &str) -> Result<()> {
        let saved_session = self.persistence.resume_session(id).await?;

        // Deserialize messages
        self.messages = serde_json::from_str(&saved_session.messages)
            .context("Failed to deserialize messages")?;

        self.current_session_id = Some(id.to_string());

        Ok(())
    }

    /// List all saved chat sessions
    pub async fn list_chats(&self) -> Result<String> {
        let sessions = self.persistence.list_sessions().await?;

        let mut output = String::new();
        output.push_str("💬 Saved Chat Sessions\n");
        output.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

        if sessions.is_empty() {
            output.push_str("No saved sessions found.\n");
        } else {
            for session in sessions {
                let name = session.name.unwrap_or_else(|| "Unnamed".to_string());
                let created = session.created_at.format("%Y-%m-%d %H:%M");
                output.push_str(&format!(
                    "📝 {} (ID: {})\n   Created: {}\n   Path: {}\n\n",
                    name, session.id, created, session.project_path
                ));
            }
        }

        Ok(output)
    }

    /// Delete a saved chat session
    pub async fn delete_chat(&self, id: &str) -> Result<()> {
        self.persistence.delete_session(id).await
    }

    /// Share a chat session (placeholder - could upload to pastebin/gist)
    pub async fn share_chat(&self, _id: &str) -> Result<String> {
        // TODO: Implement actual sharing (upload to gist, pastebin, etc.)
        Ok("Sharing not yet implemented. Save the session locally for now.".to_string())
    }

    /// Switch to a different model
    pub async fn switch_model(&mut self, model: &str) -> Result<()> {
        self.config.llm.model = model.to_string();
        self.llm_client = Arc::from(create_client(&self.config).await?);
        Ok(())
    }

    /// Get current model name
    pub fn get_current_model(&self) -> String {
        self.config.llm.model.clone()
    }

    /// List available models for the current provider
    pub async fn list_available_models(&self) -> Result<String> {
        use crate::config::LlmProvider;

        match &self.config.llm.provider {
            LlmProvider::GitHubCopilot => {
                // Get the stored GitHub token
                let token_path = Config::token_path(&LlmProvider::GitHubCopilot)?;
                if !token_path.exists() {
                    return Ok("Not logged in to GitHub Copilot. Run /login to authenticate.".to_string());
                }

                let stored_token = crate::auth::StoredToken::load(&token_path)?;
                let github_token = stored_token.get_access_token();

                // Get Copilot token from GitHub token
                let copilot_token = crate::llm::copilot::get_copilot_token(github_token).await?;

                // Fetch available models
                let models = crate::llm::copilot::get_copilot_models(&copilot_token).await?;

                let mut output = String::from("📋 Available GitHub Copilot Models:\n\n");
                let current_model = &self.config.llm.model;

                for model in models {
                    let marker = if model.id == *current_model { " ← current" } else { "" };
                    let preview = if model.preview.unwrap_or(false) { " (preview)" } else { "" };
                    output.push_str(&format!("  • {}{}{}\n", model.id, preview, marker));
                }

                output.push_str("\nUse /model <name> to switch models.");
                Ok(output)
            }
            LlmProvider::Anthropic => {
                let mut output = String::from("📋 Available Anthropic Models:\n\n");
                let current_model = &self.config.llm.model;
                let models = [
                    "claude-opus-4-20250514",
                    "claude-sonnet-4-20250514",
                    "claude-3-5-sonnet-20241022",
                    "claude-3-5-haiku-20241022",
                    "claude-3-opus-20240229",
                    "claude-3-haiku-20240307",
                ];
                for model in models {
                    let marker = if model == current_model { " ← current" } else { "" };
                    output.push_str(&format!("  • {}{}\n", model, marker));
                }
                output.push_str("\nUse /model <name> to switch models.");
                Ok(output)
            }
            LlmProvider::OpenAI => {
                let mut output = String::from("📋 Available OpenAI Models:\n\n");
                let current_model = &self.config.llm.model;
                let models = ["gpt-4o", "gpt-4o-mini", "gpt-4-turbo", "gpt-4", "gpt-3.5-turbo"];
                for model in models {
                    let marker = if model == current_model { " ← current" } else { "" };
                    output.push_str(&format!("  • {}{}\n", model, marker));
                }
                output.push_str("\nUse /model <name> to switch models.");
                Ok(output)
            }
            LlmProvider::OpenRouter => {
                Ok("📋 OpenRouter Models:\n\nOpenRouter supports many models. Visit https://openrouter.ai/models for the full list.\n\nUse /model <provider/model-name> to switch models.".to_string())
            }
            LlmProvider::Ollama => {
                Ok("📋 Ollama Models:\n\nRun `ollama list` to see installed models.\n\nUse /model <name> to switch models.".to_string())
            }
        }
    }

    /// Set approval mode
    pub fn set_approval_mode(&mut self, mode: &str) -> Result<()> {
        self.approval_mode = ApprovalMode::from_str(mode)?;
        Ok(())
    }

    /// Get current approval mode
    pub fn get_approval_mode(&self) -> String {
        self.approval_mode.to_string()
    }

    /// Restore file from checkpoint
    pub async fn restore_file(&mut self, file: Option<&str>) -> Result<()> {
        match file {
            Some(f) => {
                let path = PathBuf::from(f);
                self.checkpoints.restore_file(&path).await?;
            }
            None => {
                // Restore all files
                self.checkpoints.restore_all().await?;
            }
        }
        Ok(())
    }

    // ========== Directory Checkpoint Management ==========

    /// List all directory checkpoints
    pub async fn list_dir_checkpoints(&self) -> Result<String> {
        use crate::checkpoint::DirectoryCheckpointManager;
        let checkpoints = self.dir_checkpoints.list_checkpoints().await?;
        Ok(DirectoryCheckpointManager::format_checkpoint_list(
            &checkpoints,
        ))
    }

    /// Restore to a specific directory checkpoint
    pub async fn restore_dir_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        self.dir_checkpoints.restore_checkpoint(checkpoint_id).await
    }

    /// Restore to the latest directory checkpoint
    pub async fn restore_latest_checkpoint(&self) -> Result<()> {
        self.dir_checkpoints.restore_latest().await
    }

    /// Delete a specific directory checkpoint
    pub async fn delete_dir_checkpoint(&mut self, checkpoint_id: &str) -> Result<()> {
        self.dir_checkpoints.delete_checkpoint(checkpoint_id).await
    }

    /// Generate project summary
    pub async fn generate_project_summary(&self) -> Result<String> {
        let sandbox_dir = self.get_sandbox_dir()?;

        // Use read_file tool to scan project structure
        // This is a simplified version - could be enhanced
        let mut summary = String::new();
        summary.push_str("📊 Project Summary\n");
        summary.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");
        summary.push_str(&format!("Project Path: {}\n\n", sandbox_dir.display()));

        // TODO: Implement actual project analysis
        summary.push_str("Note: Full project analysis coming soon.\n");
        summary.push_str("For now, use the AI to analyze your codebase.\n");

        Ok(summary)
    }

    /// Compress conversation to save tokens
    pub async fn compress_conversation(&mut self) -> Result<()> {
        // Simple compression: keep only the last N messages
        const MAX_MESSAGES: usize = 20;

        if self.messages.len() > MAX_MESSAGES {
            let compressed_count = self.messages.len() - MAX_MESSAGES;
            self.messages = self.messages.split_off(compressed_count);
            tracing::info!("Compressed {} messages", compressed_count);
        }

        Ok(())
    }

    /// Get current settings
    pub fn get_settings(&self) -> String {
        let mut output = String::new();
        output.push_str("⚙️  Current Settings\n");
        output.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

        output.push_str(&format!("Model: {}\n", self.config.llm.model));
        output.push_str(&format!("Provider: {:?}\n", self.config.llm.provider));
        output.push_str(&format!("Approval Mode: {}\n", self.approval_mode));
        output.push_str(&format!("Max Tokens: {}\n", self.config.llm.max_tokens));
        output.push_str(&format!(
            "Git Auto-Commit: {}\n",
            self.config.git.auto_commit
        ));
        output.push_str(&format!("Project: {}\n", self.project_path.display()));

        output.push_str("\n--- Tool Settings ---\n");
        output.push_str(&format!(
            "Bash Timeout: {}s\n",
            self.config.tools.bash_timeout_secs
        ));
        output.push_str(&format!(
            "Max Output Size: {} bytes ({:.1} MB)\n",
            self.config.tools.max_output_bytes,
            self.config.tools.max_output_bytes as f64 / 1_048_576.0
        ));
        output.push_str(&format!(
            "Dangerous Command Warnings: {}\n",
            if self.config.tools.warn_dangerous_commands {
                "enabled"
            } else {
                "disabled"
            }
        ));

        output
    }

    /// List available tools
    pub fn list_tools(&self) -> String {
        let mut output = String::new();
        output.push_str("🔧 Available Tools\n");
        output.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

        let schemas = self.tool_registry.get_tools_schema();
        for schema in schemas {
            if let Some(name) = schema["name"].as_str() {
                let desc = schema["description"].as_str().unwrap_or("No description");
                output.push_str(&format!("• {}: {}\n", name, desc));
            }
        }

        output
    }

    /// Copy last output to clipboard (placeholder)
    pub fn copy_last_output(&self) -> Result<()> {
        // TODO: Implement clipboard support using `arboard` or `clipboard` crate
        tracing::warn!("Clipboard support not yet implemented");
        Ok(())
    }

    /// Add memory/instruction
    pub async fn add_memory(&mut self, content: &str) -> Result<()> {
        self.memory.add_instruction(content.to_string());
        Ok(())
    }

    /// Show current memory
    pub async fn show_memory(&self) -> Result<String> {
        self.memory.show().await
    }

    /// Refresh memory from file
    pub async fn refresh_memory(&mut self) -> Result<()> {
        self.memory.refresh().await
    }

    /// Initialize project context file
    pub async fn init_project_context(&self) -> Result<()> {
        self.memory.init_file().await
    }

    /// Add directory to workspace (placeholder)
    pub async fn add_directory(&mut self, _path: &str) -> Result<()> {
        // TODO: Implement multi-directory workspace support
        Ok(())
    }

    /// List workspace directories
    pub async fn list_directories(&self) -> Result<String> {
        Ok(format!(
            "Current directory: {}",
            self.project_path.display()
        ))
    }

    // ========== Undo/Redo Support ==========

    /// Undo the last change using git
    pub async fn undo(&mut self) -> Result<String> {
        let result = self.git_manager.undo().await?;
        Ok(result.format())
    }

    /// Redo a previously undone change
    pub async fn redo(&mut self) -> Result<String> {
        let result = self.git_manager.redo().await?;
        Ok(result.format())
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        self.git_manager.can_redo()
    }

    // ========== Manual Context Compaction ==========

    /// Manually trigger context compaction
    pub async fn compact_context(&mut self) -> Result<String> {
        let stats_before = self.context_manager.analyze(&self.messages);

        // Force compaction even if not at threshold
        let (compacted, result) = self
            .context_manager
            .compact(std::mem::take(&mut self.messages));

        self.messages = compacted;

        let stats_after = self.context_manager.analyze(&self.messages);

        let mut output = String::new();
        output.push_str("📦 Context Compacted\n");
        output.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");
        output.push_str(&format!(
            "Before: ~{} tokens ({} messages)\n",
            result.tokens_before, stats_before.message_count
        ));
        output.push_str(&format!(
            "After:  ~{} tokens ({} messages)\n",
            result.tokens_after, stats_after.message_count
        ));
        output.push_str(&format!("Saved:  ~{} tokens\n", result.tokens_saved()));

        if result.did_compact() {
            output.push_str(&format!("\nSummary: {}\n", result.summary));
        }

        Ok(output)
    }
}

/// Parse a plan from LLM response text
///
/// Expected format:
/// ```
/// ## Plan: [Title]
/// 1. [Step description]
/// 2. [Step description]
/// ...
/// ```
fn parse_plan_from_response(response: &str) -> (String, Vec<PlanStep>) {
    let mut title = "Untitled Plan".to_string();
    let mut steps = Vec::new();

    // Try to find "## Plan: [title]" or just "## [title]"
    for line in response.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## Plan:") {
            title = trimmed.trim_start_matches("## Plan:").trim().to_string();
        } else if trimmed.starts_with("## ") && title == "Untitled Plan" {
            // Fallback: use any ## header as title
            title = trimmed.trim_start_matches("## ").trim().to_string();
        }
    }

    // Parse numbered steps (e.g., "1. Do something", "2. Do another thing")
    let step_regex = regex::Regex::new(r"^\s*(\d+)\.\s+(.+)$").unwrap();
    for line in response.lines() {
        if let Some(caps) = step_regex.captures(line) {
            if let Some(description) = caps.get(2) {
                let desc_text = description.as_str().trim().to_string();
                let step_id = format!("step-{}", steps.len() + 1);
                steps.push(PlanStep::new(step_id, desc_text));
            }
        }
    }

    // If no numbered steps found, try to extract bullet points
    if steps.is_empty() {
        for line in response.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                let desc_text = trimmed[2..].trim().to_string();
                let step_id = format!("step-{}", steps.len() + 1);
                steps.push(PlanStep::new(step_id, desc_text));
            }
        }
    }

    // If still no steps found, create a generic step
    if steps.is_empty() {
        steps.push(PlanStep::new("step-1".to_string(), "Execute the plan".to_string()));
    }

    (title, steps)
}
