use anyhow::{Context, Result};
use chrono::Utc;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::approval::{ApprovalMode, ExecutionMode, ExecutionPlan, PlannedTool};
use crate::checkpoint::CheckpointManager;
use crate::config::Config;
use crate::context::ContextManager;
use crate::custom_commands::CustomCommandManager;
use crate::git::GitManager;
use crate::llm::{create_client, ContentBlock, LlmClient, Message, ToolDefinition};
use crate::loop_detector::{DoomLoopAction, LoopDetector};
use crate::memory::MemoryManager;
use crate::permissions::PermissionManager;
use crate::persistence::{SessionPersistence, SessionStats, ToolUsage};
use crate::prompts;
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
    custom_commands: CustomCommandManager,
    session_start: chrono::DateTime<Utc>,
    current_session_id: Option<String>,
    last_output: String,
}

impl Session {
    pub async fn new(config: Config, project_path: PathBuf) -> Result<Self> {
        let llm_client = create_client(&config).await?;
        let tool_registry = ToolRegistry::new();

        // Initialize git for safety
        let git_manager = GitManager::new(project_path.clone());

        // Initialize new features
        let persistence = SessionPersistence::new().await?;
        let memory = MemoryManager::new(project_path.clone());
        let custom_commands = CustomCommandManager::new(project_path.clone()).await?;
        let checkpoints = CheckpointManager::new(project_path.clone());

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
            custom_commands,
            session_start: Utc::now(),
            current_session_id: None,
            last_output: String::new(),
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
            let assistant_message = self
                .llm_client
                .send_message_with_system(&self.messages, &tools, Some(&system_prompt))
                .await?;

            // Track stats (approximate token counting)
            self.stats.total_tokens_sent += user_message.len() / 4; // Rough estimate
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
                self.messages.push(Message {
                    role: crate::llm::Role::User,
                    content: tool_results,
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
                let _ = event_tx.send(SessionEvent::TextChunk(format!(
                    "\n📦 Context compacted: {}\n",
                    summary
                )));
            }
        }

        let mut response_text = String::new();

        // Build hierarchical system prompt
        let project_context = self.memory.get_system_prompt().await.ok();
        let system_prompt =
            prompts::build_system_prompt(self.agent_mode, project_context.as_deref(), None);

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
            let assistant_message = self
                .llm_client
                .send_message_with_system(&self.messages, &tools, Some(&system_prompt))
                .await?;

            // Track stats
            self.stats.total_tokens_sent += user_message.len() / 4;
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
                    } else {
                        ToolContext::new(&self.project_path, &self.config.tools)
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
                        if let Some(old) = old_content {
                            let path_key = if name == "edit_file" {
                                "file_path"
                            } else {
                                "path"
                            };
                            if let Some(path) = input.get(path_key).and_then(|v| v.as_str()) {
                                let full_path = self.project_path.join(path);
                                if let Ok(new_content) = std::fs::read_to_string(&full_path) {
                                    let _ = event_tx.send(SessionEvent::FileDiff {
                                        path: path.to_string(),
                                        old_content: old,
                                        new_content,
                                    });
                                }
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

                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: result,
                    });
                }
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
                self.messages.push(Message {
                    role: crate::llm::Role::User,
                    content: tool_results,
                });
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
}
