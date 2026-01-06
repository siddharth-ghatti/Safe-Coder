use anyhow::{Context, Result};
use chrono::Utc;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::approval::{ApprovalMode, ExecutionMode, ExecutionPlan, PlannedTool};
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
use crate::planning::{PlanEvent, PlanStatus, PlanStep, PlanStepStatus, TaskPlan};
use crate::prompts;
use crate::tools::todo::{get_todo_list, increment_turns_without_update, should_show_reminder};
use crate::tools::{AgentMode, ToolContext, ToolRegistry};

/// Events emitted during AI message processing for real-time UI updates
#[derive(Debug, Clone)]
pub enum SessionEvent {
    /// AI is thinking/processing
    Thinking(String),
    /// Tool execution started
    ToolStart { name: String, description: String },
    /// Tool produced output
    ToolOutput { name: String, output: String },
    /// Streaming output line from bash command (for inline display)
    BashOutputLine { name: String, line: String },
    /// Tool execution completed
    ToolComplete { name: String, success: bool },
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
}

pub struct Session {
    config: Config,
    llm_client: Box<dyn LlmClient>,
    tool_registry: ToolRegistry,
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
    execution_mode: ExecutionMode,
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
        let llm_client = create_client(&config).await?;

        // Initialize tool registry with subagent support
        let mut tool_registry = if let Some(ref tx) = event_tx {
            ToolRegistry::new()
                .with_subagent_support_and_events(config.clone(), project_path.clone(), tx.clone())
                .await
        } else {
            ToolRegistry::new()
                .with_subagent_support(config.clone(), project_path.clone())
                .await
        };

        // Initialize MCP manager and register its tools
        let mut mcp_manager = McpManager::new(config.mcp.clone());
        mcp_manager.initialize(&project_path).await?;

        // Register MCP tools with the tool registry
        for tool in mcp_manager.get_tools() {
            tool_registry.register(tool);
        }

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

        Ok(Self {
            config,
            llm_client,
            tool_registry,
            messages: vec![],
            project_path: project_path.clone(),

            git_manager,
            loop_detector: LoopDetector::new(),
            context_manager: ContextManager::new(),
            permission_manager: PermissionManager::new(),

            persistence,
            approval_mode: ApprovalMode::default(),
            execution_mode: ExecutionMode::default(),
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
        })
    }

    /// Set execution mode (Plan or Act)
    pub fn set_execution_mode(&mut self, mode: ExecutionMode) {
        self.execution_mode = mode;
        tracing::info!("Execution mode set to: {}", mode);
    }

    /// Get current execution mode
    pub fn execution_mode(&self) -> ExecutionMode {
        self.execution_mode
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

    /// Apply a permission preset (safe, dev, full, yolo)
    pub fn apply_permission_preset(&mut self, preset: &str) {
        self.permission_manager.apply_preset(preset);
        tracing::info!("Applied permission preset: {}", preset);
    }

    /// Get permission manager summary
    pub fn permission_summary(&self) -> String {
        self.permission_manager.summary()
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

    pub async fn send_message(&mut self, user_message: String) -> Result<String> {
        // Create checkpoint before processing user task (git-agnostic safety)
        if self.dir_checkpoints.is_enabled() {
            let label = user_message.chars().take(100).collect::<String>();
            if let Err(e) = self.dir_checkpoints.create_checkpoint(&label).await {
                tracing::warn!("Failed to create checkpoint: {}", e);
            }
        }

        // Track stats
        self.stats.total_messages += 1;

        // Add user message to history
        self.messages.push(Message::user(user_message.clone()));

        // Check if context compaction is needed
        if self.context_manager.needs_compaction(&self.messages) {
            let (compacted, summary) = self
                .context_manager
                .compact(std::mem::take(&mut self.messages));
            self.messages = compacted;
            if !summary.is_empty() {
                tracing::info!("Context compacted: {}", summary);
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

            // Send to LLM with hierarchical system prompt
            let llm_response = self
                .llm_client
                .send_message_with_system(&self.messages, &tools, Some(&system_prompt))
                .await?;

            let assistant_message = llm_response.message;

            // Track stats from actual token usage if available
            if let Some(usage) = &llm_response.usage {
                self.stats.total_tokens_sent += usage.input_tokens;
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
            for block in &assistant_message.content {
                if let ContentBlock::Text { text } = block {
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

            // Handle based on execution mode
            match self.execution_mode {
                ExecutionMode::Plan => {
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
                ExecutionMode::Act => {
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
                        DoomLoopAction::Warn { message } | DoomLoopAction::AskUser { message } => {
                            tracing::warn!("{}", message);
                        }
                        DoomLoopAction::Continue => {}
                    }

                    // Create tool context with working directory and config
                    let tool_ctx = ToolContext::new(&self.project_path, &self.config.tools);

                    let (result, success) = match self.tool_registry.get_tool(name) {
                        Some(tool) => match tool.execute(input.clone(), &tool_ctx).await {
                            Ok(output) => {
                                tools_executed.push(name.clone());
                                (output, true)
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
                        content: result,
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
                    // Run build verification after file modifications
                    if let Some(build_errors) = self.verify_build().await {
                        final_results.push(ContentBlock::Text {
                            text: format!(
                                "\n\n--- Build Verification Failed ---\nThe code does not compile. Fix these errors before proceeding:\n{}",
                                build_errors
                            ),
                        });
                    }

                    // Check for LSP diagnostics after file modifications
                    let diagnostics_summary = self.lsp_manager.get_diagnostics_summary().await;
                    if !diagnostics_summary.is_empty() {
                        final_results.push(ContentBlock::Text {
                            text: format!(
                                "\n\n--- LSP Diagnostics ---\nThe following issues were detected after your changes:\n{}",
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

        // Add user message to history
        self.messages.push(Message::user(user_message.clone()));

        // Check if context compaction is needed
        if self.context_manager.needs_compaction(&self.messages) {
            // Get stats before compaction to calculate tokens compressed
            let stats_before = self.context_manager.analyze(&self.messages);
            let (compacted, summary) = self
                .context_manager
                .compact(std::mem::take(&mut self.messages));
            let stats_after = self.context_manager.analyze(&compacted);
            let tokens_compressed = stats_before
                .estimated_tokens
                .saturating_sub(stats_after.estimated_tokens);

            self.messages = compacted;
            if !summary.is_empty() {
                tracing::info!("Context compacted: {}", summary);
                let _ = event_tx.send(SessionEvent::TextChunk(format!(
                    "\n📦 Context compacted: {}\n",
                    summary
                )));
                // Send compression event for sidebar token tracking
                let _ = event_tx.send(SessionEvent::ContextCompressed { tokens_compressed });
            }
        }

        let mut response_text = String::new();

        // Build hierarchical system prompt
        let project_context = self.memory.get_system_prompt().await.ok();
        let system_prompt =
            prompts::build_system_prompt(self.agent_mode, project_context.as_deref(), None);

        // Create a persistent plan ID for this task - will accumulate steps across LLM calls
        let task_plan_id = format!("plan-{}", uuid::Uuid::new_v4().to_string()[..8].to_string());
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

            // Send to LLM with hierarchical system prompt
            let llm_response = self
                .llm_client
                .send_message_with_system(&self.messages, &tools, Some(&system_prompt))
                .await?;

            let assistant_message = llm_response.message;

            // Track stats and emit token usage event
            if let Some(usage) = &llm_response.usage {
                self.stats.total_tokens_sent += usage.input_tokens;
                // Emit token usage event for sidebar (including cache stats)
                let _ = event_tx.send(SessionEvent::TokenUsage {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                    cache_read_tokens: usage.cache_read_tokens,
                    cache_creation_tokens: usage.cache_creation_tokens,
                });
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

            // Extract text from response and send as chunks
            for block in &assistant_message.content {
                if let ContentBlock::Text { text } = block {
                    response_text.push_str(text);
                    response_text.push('\n');
                    let _ = event_tx.send(SessionEvent::TextChunk(text.clone()));
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
                            // For now, treat as warning (full approval would require UI changes)
                            tracing::warn!("{}", message);
                            let _ =
                                event_tx.send(SessionEvent::TextChunk(format!("\n{}\n", message)));
                        }
                        DoomLoopAction::Continue => {}
                    }

                    // Generate description for the tool action
                    let description = self.describe_tool_action(name, input);

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

                    let (result, success) = match self.tool_registry.get_tool(name) {
                        Some(tool) => match tool.execute(input.clone(), &tool_ctx).await {
                            Ok(output) => {
                                tools_executed.push(name.clone());
                                (output, true)
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
                        // Check for failure loop
                        if let Some(DoomLoopAction::AskUser { message }) =
                            self.loop_detector.check_failure_loop()
                        {
                            let _ =
                                event_tx.send(SessionEvent::TextChunk(format!("\n{}\n", message)));
                        }
                    }

                    // For edit_file, send diff if we have old content
                    // Note: edit_file uses "file_path", write_file uses "path"
                    if (name == "edit_file" || name == "write_file") && success {
                        let path_key = if name == "edit_file" {
                            "file_path"
                        } else {
                            "path"
                        };
                        if let Some(path) = input.get(path_key).and_then(|v| v.as_str()) {
                            let full_path = self.project_path.join(path);

                            // Send diff event if we have old content
                            if let Some(old) = old_content {
                                if let Ok(new_content) = std::fs::read_to_string(&full_path) {
                                    let _ = event_tx.send(SessionEvent::FileDiff {
                                        path: path.to_string(),
                                        old_content: old,
                                        new_content: new_content.clone(),
                                    });
                                }
                            }

                            // Notify LSP of file change for diagnostics
                            if let Err(e) = self.lsp_manager.notify_file_changed(&full_path).await {
                                tracing::debug!("LSP file change notification failed: {}", e);
                            }
                        }
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
                        content: result,
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
                    // Run build verification after file modifications
                    if let Some(build_errors) = self.verify_build().await {
                        has_issues = true;
                        final_results.push(ContentBlock::Text {
                            text: format!(
                                "\n\n--- Build Verification Failed ---\nThe code does not compile. Fix these errors before proceeding:\n{}",
                                build_errors
                            ),
                        });

                        let _ = event_tx.send(SessionEvent::TextChunk(
                            "\n❌ Build failed - errors detected\n".to_string(),
                        ));
                    }

                    // Check for LSP diagnostics after file modifications
                    let diagnostics_summary = self.lsp_manager.get_diagnostics_summary().await;
                    if !diagnostics_summary.is_empty() {
                        has_issues = true;
                        final_results.push(ContentBlock::Text {
                            text: format!(
                                "\n\n--- LSP Diagnostics ---\nThe following issues were detected after your changes:\n{}",
                                diagnostics_summary
                            ),
                        });

                        let _ = event_tx.send(SessionEvent::TextChunk(
                            "\n⚠️ LSP detected issues - see diagnostics above\n".to_string(),
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

    /// Detect the project type and return the appropriate build command
    fn detect_build_command(&self) -> Option<&'static str> {
        // Check for Rust project
        if self.project_path.join("Cargo.toml").exists() {
            return Some("cargo build 2>&1");
        }
        // Check for Node.js/TypeScript project
        if self.project_path.join("package.json").exists() {
            // Check if there's a build script
            if let Ok(content) = std::fs::read_to_string(self.project_path.join("package.json")) {
                if content.contains("\"build\"") {
                    return Some("npm run build 2>&1");
                }
                // TypeScript check
                if content.contains("\"tsc\"") || self.project_path.join("tsconfig.json").exists() {
                    return Some("npx tsc --noEmit 2>&1");
                }
            }
        }
        // Check for Go project
        if self.project_path.join("go.mod").exists() {
            return Some("go build ./... 2>&1");
        }
        // Check for Python project
        if self.project_path.join("pyproject.toml").exists()
            || self.project_path.join("setup.py").exists()
        {
            // Python doesn't have a traditional "build" but we can type-check
            if self.project_path.join("pyproject.toml").exists() {
                return Some("python -m py_compile $(find . -name '*.py' -not -path './venv/*' | head -20) 2>&1");
            }
        }
        None
    }

    /// Run build verification and return any errors
    ///
    /// This runs the project's build command and returns the output if there are errors.
    /// Returns None if build succeeds or if no build command is available.
    pub async fn verify_build(&self) -> Option<String> {
        let build_cmd = self.detect_build_command()?;

        // Run the build command
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(build_cmd)
            .current_dir(&self.project_path)
            .output()
            .await
            .ok()?;

        if output.status.success() {
            None
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            let combined = format!("{}{}", stdout, stderr);

            // Only return if there's meaningful output
            if combined.trim().is_empty() {
                Some("Build failed with no output".to_string())
            } else {
                // Limit output to avoid overwhelming the context
                let truncated = if combined.len() > 2000 {
                    format!("{}...\n[output truncated]", &combined[..2000])
                } else {
                    combined.to_string()
                };
                Some(truncated)
            }
        }
    }

    /// Generate a human-readable description of a tool action
    fn describe_tool_action(&self, name: &str, params: &serde_json::Value) -> String {
        match name {
            "read_file" => {
                if let Some(path) = params.get("path").and_then(|v| v.as_str()) {
                    format!("Read file: {}", path)
                } else {
                    "Read a file".to_string()
                }
            }
            "write_file" => {
                if let Some(path) = params.get("path").and_then(|v| v.as_str()) {
                    let content_preview = params
                        .get("content")
                        .and_then(|v| v.as_str())
                        .map(|c| {
                            if c.len() > 50 {
                                format!("{}...", &c[..50])
                            } else {
                                c.to_string()
                            }
                        })
                        .unwrap_or_default();
                    format!("Write to {}: {}", path, content_preview)
                } else {
                    "Write a file".to_string()
                }
            }
            "edit_file" => {
                if let Some(path) = params.get("file_path").and_then(|v| v.as_str()) {
                    format!("Edit file: {}", path)
                } else {
                    "Edit a file".to_string()
                }
            }
            "bash" => {
                if let Some(cmd) = params.get("command").and_then(|v| v.as_str()) {
                    let cmd_preview = if cmd.len() > 60 {
                        format!("{}...", &cmd[..60])
                    } else {
                        cmd.to_string()
                    };
                    format!("Run command: {}", cmd_preview)
                } else {
                    "Execute bash command".to_string()
                }
            }
            _ => format!("Execute {}", name),
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
        self.llm_client = create_client(&self.config).await?;
        Ok(())
    }

    /// Get current model name
    pub fn get_current_model(&self) -> String {
        self.config.llm.model.clone()
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
        let (compacted, summary) = self
            .context_manager
            .compact(std::mem::take(&mut self.messages));

        let stats_after = self.context_manager.analyze(&compacted);
        let tokens_saved = stats_before
            .estimated_tokens
            .saturating_sub(stats_after.estimated_tokens);

        self.messages = compacted;

        let mut output = String::new();
        output.push_str("📦 Context Compacted\n");
        output.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");
        output.push_str(&format!(
            "Before: ~{} tokens ({} messages)\n",
            stats_before.estimated_tokens, stats_before.message_count
        ));
        output.push_str(&format!(
            "After:  ~{} tokens ({} messages)\n",
            stats_after.estimated_tokens, stats_after.message_count
        ));
        output.push_str(&format!("Saved:  ~{} tokens\n", tokens_saved));

        if !summary.is_empty() {
            output.push_str(&format!("\nSummary: {}\n", summary));
        }

        Ok(output)
    }
}
