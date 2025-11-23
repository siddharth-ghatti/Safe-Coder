use anyhow::{Context, Result};
use chrono::Utc;
use std::path::PathBuf;

use crate::approval::ApprovalMode;
use crate::checkpoint::CheckpointManager;
use crate::config::Config;
use crate::custom_commands::CustomCommandManager;
use crate::git::GitManager;
use crate::llm::{ContentBlock, LlmClient, Message, ToolDefinition, create_client};
use crate::memory::MemoryManager;
use crate::persistence::{SessionPersistence, SessionStats, ToolUsage};
use crate::tools::ToolRegistry;

pub struct Session {
    config: Config,
    llm_client: Box<dyn LlmClient>,
    tool_registry: ToolRegistry,
    messages: Vec<Message>,
    project_path: PathBuf,

    // Safety & tracking
    git_manager: GitManager,

    // Features
    persistence: SessionPersistence,
    approval_mode: ApprovalMode,
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
        let llm_client = create_client(&config.llm)?;
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

            persistence,
            approval_mode: ApprovalMode::default(),
            stats: SessionStats::new(),
            memory,
            checkpoints,
            custom_commands,
            session_start: Utc::now(),
            current_session_id: None,
            last_output: String::new(),
        })
    }

    pub async fn start(&mut self) -> Result<()> {
        tracing::info!("🔒 Starting Safe Coder session for project: {:?}", self.project_path);

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

        let mut response_text = String::new();

        loop {
            // Get tools schema
            let tools: Vec<ToolDefinition> = self.tool_registry
                .get_tools_schema()
                .into_iter()
                .map(|schema| ToolDefinition {
                    name: schema["name"].as_str().unwrap().to_string(),
                    description: schema["description"].as_str().unwrap().to_string(),
                    input_schema: schema["input_schema"].clone(),
                })
                .collect();

            // Send to LLM
            let assistant_message = self.llm_client
                .send_message(&self.messages, &tools)
                .await?;

            // Track stats (approximate token counting)
            self.stats.total_tokens_sent += user_message.len() / 4; // Rough estimate
            self.stats.total_messages += 1;

            // Check if there are any tool calls
            let has_tool_calls = assistant_message.content.iter()
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

            // Execute tool calls
            let mut tool_results = Vec::new();
            let mut tools_executed = Vec::new();

            for block in &assistant_message.content {
                if let ContentBlock::ToolUse { id, name, input } = block {
                    // Track stats
                    self.stats.total_tool_calls += 1;

                    // Update tool usage stats
                    let tool_stat = self.stats.tools_used
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

                    let working_dir = &self.project_path;

                    let result = match self.tool_registry.get_tool(name) {
                        Some(tool) => {
                            match tool.execute(input.clone(), working_dir).await {
                                Ok(output) => {
                                    tools_executed.push(name.clone());
                                    output
                                }
                                Err(e) => format!("Error: {}", e),
                            }
                        }
                        None => format!("Error: Unknown tool '{}'", name),
                    };

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
        let bash_tool = self.tool_registry.get_tool("bash")
            .context("Bash tool not found")?;

        let input = serde_json::json!({
            "command": command
        });

        bash_tool.execute(input, &self.project_path).await
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
        let id = self.persistence
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
        self.llm_client = create_client(&self.config.llm)?;
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
            },
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
        output.push_str(&format!("Git Auto-Commit: {}\n", self.config.git.auto_commit));
        output.push_str(&format!("Project: {}\n", self.project_path.display()));

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
        Ok(format!("Current directory: {}", self.project_path.display()))
    }
}
