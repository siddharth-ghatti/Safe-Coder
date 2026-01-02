//! Subagent Executor
//!
//! Runs a bounded conversation loop for a subagent, executing tools
//! and streaming progress back to the parent.

use anyhow::Result;
use std::path::PathBuf;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::config::Config;
use crate::context::{ContextConfig, ContextManager};
use crate::llm::{create_client, ContentBlock, LlmClient, Message, ToolDefinition};
use crate::tools::{ToolContext, ToolRegistry};

use super::prompts::build_subagent_prompt;
use super::types::{SubagentEvent, SubagentKind, SubagentResult, SubagentScope};

/// Executor that runs a subagent's conversation loop
pub struct SubagentExecutor {
    /// Unique ID for this subagent instance
    id: String,
    /// Kind of subagent
    kind: SubagentKind,
    /// Scope and configuration
    scope: SubagentScope,
    /// Project path
    project_path: PathBuf,
    /// LLM client
    llm_client: Box<dyn LlmClient>,
    /// Tool registry
    tool_registry: ToolRegistry,
    /// Tool config
    tool_config: crate::config::ToolConfig,
    /// Event channel for progress updates
    event_tx: mpsc::UnboundedSender<SubagentEvent>,
    /// Message history
    messages: Vec<Message>,
    /// Context manager for compaction
    context_manager: ContextManager,
    /// Files read during execution
    files_read: Vec<String>,
    /// Files modified during execution
    files_modified: Vec<String>,
}

impl SubagentExecutor {
    /// Create a new subagent executor
    pub async fn new(
        kind: SubagentKind,
        scope: SubagentScope,
        project_path: PathBuf,
        config: &Config,
        event_tx: mpsc::UnboundedSender<SubagentEvent>,
    ) -> Result<Self> {
        let id = format!("subagent-{}", Uuid::new_v4().to_string()[..8].to_string());
        let llm_client = create_client(config).await?;
        // Subagents don't spawn other subagents - use registry without subagent support
        let tool_registry = ToolRegistry::new_without_subagents();

        // Create context manager with smaller limits for subagents
        // Subagents should be more aggressive about compaction since they're focused tasks
        let context_config = ContextConfig {
            max_tokens: 80_000,           // Smaller window for subagents
            compact_threshold_pct: 40,    // Compact earlier (at 40%)
            preserve_recent_messages: 10, // Keep fewer messages
            preserve_tool_results: 5,     // Keep fewer tool results
            chars_per_token: 4,
        };
        let context_manager = ContextManager::with_config(context_config);

        Ok(Self {
            id,
            kind,
            scope,
            project_path,
            llm_client,
            tool_registry,
            tool_config: config.tools.clone(),
            event_tx,
            messages: Vec::new(),
            context_manager,
            files_read: Vec::new(),
            files_modified: Vec::new(),
        })
    }

    /// Get the subagent ID
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Execute the subagent task
    pub async fn execute(&mut self) -> Result<SubagentResult> {
        // Send started event
        let _ = self.event_tx.send(SubagentEvent::Started {
            id: self.id.clone(),
            kind: self.kind.clone(),
            task: self.scope.task.clone(),
        });

        // Build system prompt
        let system_prompt = build_subagent_prompt(&self.kind, &self.scope);

        // Add initial user message with the task
        self.messages.push(Message::user(format!(
            "Please complete this task: {}",
            self.scope.task
        )));

        let mut iteration = 0;
        let mut final_response = String::new();
        let mut errors = Vec::new();

        // Main conversation loop
        loop {
            iteration += 1;

            // Check iteration limit
            if iteration > self.scope.max_iterations {
                let _ = self.event_tx.send(SubagentEvent::Error {
                    id: self.id.clone(),
                    error: format!("Reached maximum iterations ({})", self.scope.max_iterations),
                });
                break;
            }

            // Check if context needs compaction before sending to LLM
            if self.context_manager.needs_compaction(&self.messages) {
                let _ = self.event_tx.send(SubagentEvent::Thinking {
                    id: self.id.clone(),
                    message: "Compacting context...".to_string(),
                });

                let (compacted, summary) = self
                    .context_manager
                    .compact(std::mem::take(&mut self.messages));
                self.messages = compacted;

                if !summary.is_empty() {
                    let _ = self.event_tx.send(SubagentEvent::Thinking {
                        id: self.id.clone(),
                        message: format!("Context compacted: {}", summary),
                    });
                }
            }

            // Send thinking event
            let _ = self.event_tx.send(SubagentEvent::Thinking {
                id: self.id.clone(),
                message: format!("Iteration {}/{}", iteration, self.scope.max_iterations),
            });

            // Get available tools for this subagent kind
            let tools = self.get_filtered_tools();

            // Log available tools for debugging
            let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
            let _ = self.event_tx.send(SubagentEvent::Thinking {
                id: self.id.clone(),
                message: format!("Available tools: {:?}", tool_names),
            });

            // Send to LLM
            let response = match self
                .llm_client
                .send_message_with_system(&self.messages, &tools, Some(&system_prompt))
                .await
            {
                Ok(msg) => msg,
                Err(e) => {
                    errors.push(format!("LLM error: {}", e));
                    let _ = self.event_tx.send(SubagentEvent::Error {
                        id: self.id.clone(),
                        error: e.to_string(),
                    });
                    break;
                }
            };

            // Check for tool calls
            let has_tool_calls = response
                .content
                .iter()
                .any(|c| matches!(c, ContentBlock::ToolUse { .. }));

            // Count tool calls for logging
            let tool_call_count = response
                .content
                .iter()
                .filter(|c| matches!(c, ContentBlock::ToolUse { .. }))
                .count();

            let _ = self.event_tx.send(SubagentEvent::Thinking {
                id: self.id.clone(),
                message: format!("Response has {} tool calls", tool_call_count),
            });

            // Extract and send text chunks
            for block in &response.content {
                if let ContentBlock::Text { text } = block {
                    final_response = text.clone();
                    let _ = self.event_tx.send(SubagentEvent::TextChunk {
                        id: self.id.clone(),
                        text: text.clone(),
                    });
                }
            }

            // Add assistant message to history
            self.messages.push(response.clone());

            // If no tool calls, we're done
            if !has_tool_calls {
                let _ = self.event_tx.send(SubagentEvent::Thinking {
                    id: self.id.clone(),
                    message: "No tool calls in response, finishing".to_string(),
                });
                break;
            }

            // Execute tool calls
            let mut tool_results = Vec::new();

            for block in &response.content {
                if let ContentBlock::ToolUse { id, name, input } = block {
                    // Check if tool is allowed for this subagent kind
                    if !self.kind.is_tool_allowed(name) {
                        let error_msg = format!(
                            "Tool '{}' is not available for {} subagent",
                            name,
                            self.kind.display_name()
                        );
                        let _ = self.event_tx.send(SubagentEvent::ToolStart {
                            id: self.id.clone(),
                            tool_name: name.clone(),
                            description: format!("Blocked: {}", name),
                        });
                        let _ = self.event_tx.send(SubagentEvent::ToolComplete {
                            id: self.id.clone(),
                            tool_name: name.clone(),
                            success: false,
                        });
                        tool_results.push(ContentBlock::ToolResult {
                            tool_use_id: id.clone(),
                            content: error_msg,
                        });
                        continue;
                    }

                    // Generate description
                    let description = self.describe_tool_action(name, input);

                    // Send tool start event
                    let _ = self.event_tx.send(SubagentEvent::ToolStart {
                        id: self.id.clone(),
                        tool_name: name.clone(),
                        description: description.clone(),
                    });

                    // Execute the tool
                    let tool_ctx = ToolContext::new(&self.project_path, &self.tool_config);
                    let result = match self.tool_registry.get_tool(name) {
                        Some(tool) => match tool.execute(input.clone(), &tool_ctx).await {
                            Ok(output) => {
                                // Track file operations
                                self.track_file_operation(name, input);

                                // Truncate output for display (safely handle UTF-8 boundaries)
                                // Send full output for streaming display
                                let _ = self.event_tx.send(SubagentEvent::ToolOutput {
                                    id: self.id.clone(),
                                    tool_name: name.clone(),
                                    output: output.clone(),
                                });
                                let _ = self.event_tx.send(SubagentEvent::ToolComplete {
                                    id: self.id.clone(),
                                    tool_name: name.clone(),
                                    success: true,
                                });
                                output
                            }
                            Err(e) => {
                                let error_msg = format!("Tool error: {}", e);
                                errors.push(error_msg.clone());
                                let _ = self.event_tx.send(SubagentEvent::ToolOutput {
                                    id: self.id.clone(),
                                    tool_name: name.clone(),
                                    output: error_msg.clone(),
                                });
                                let _ = self.event_tx.send(SubagentEvent::ToolComplete {
                                    id: self.id.clone(),
                                    tool_name: name.clone(),
                                    success: false,
                                });
                                error_msg
                            }
                        },
                        None => {
                            let error_msg = format!("Unknown tool: {}", name);
                            let _ = self.event_tx.send(SubagentEvent::ToolComplete {
                                id: self.id.clone(),
                                tool_name: name.clone(),
                                success: false,
                            });
                            error_msg
                        }
                    };

                    tool_results.push(ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: result,
                    });
                }
            }

            // Add tool results to messages
            if !tool_results.is_empty() {
                self.messages.push(Message {
                    role: crate::llm::Role::User,
                    content: tool_results,
                });
            }

            // Send iteration complete event
            let _ = self.event_tx.send(SubagentEvent::IterationComplete {
                id: self.id.clone(),
                iteration,
                max_iterations: self.scope.max_iterations,
            });
        }

        // Build result
        let success = errors.is_empty();
        let summary = if success {
            format!(
                "{} completed task in {} iteration(s)",
                self.kind.display_name(),
                iteration
            )
        } else {
            format!(
                "{} encountered errors after {} iteration(s)",
                self.kind.display_name(),
                iteration
            )
        };

        // Send completed event
        let _ = self.event_tx.send(SubagentEvent::Completed {
            id: self.id.clone(),
            success,
            summary: summary.clone(),
        });

        Ok(SubagentResult {
            success,
            summary,
            output: final_response,
            iterations: iteration,
            files_read: self.files_read.clone(),
            files_modified: self.files_modified.clone(),
            errors,
        })
    }

    /// Get tool definitions filtered by subagent kind
    fn get_filtered_tools(&self) -> Vec<ToolDefinition> {
        self.tool_registry
            .get_tools_schema()
            .into_iter()
            .filter(|schema| {
                let name = schema["name"].as_str().unwrap_or("");
                self.kind.is_tool_allowed(name)
            })
            .map(|schema| ToolDefinition {
                name: schema["name"].as_str().unwrap().to_string(),
                description: schema["description"].as_str().unwrap().to_string(),
                input_schema: schema["input_schema"].clone(),
            })
            .collect()
    }

    /// Generate a human-readable description of a tool action
    fn describe_tool_action(&self, name: &str, input: &serde_json::Value) -> String {
        match name {
            "read_file" => {
                let path = input
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                format!("Reading {}", path)
            }
            "write_file" => {
                let path = input
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                format!("Writing {}", path)
            }
            "edit_file" => {
                let path = input
                    .get("file_path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                format!("Editing {}", path)
            }
            "list_file" => {
                let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
                format!("Listing {}", path)
            }
            "glob" => {
                let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("*");
                format!("Finding files: {}", pattern)
            }
            "grep" => {
                let pattern = input
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .unwrap_or("...");
                format!("Searching for: {}", pattern)
            }
            "bash" => {
                let cmd = input
                    .get("command")
                    .and_then(|v| v.as_str())
                    .unwrap_or("...");
                let short_cmd = if cmd.len() > 50 {
                    format!("{}...", &cmd[..50])
                } else {
                    cmd.to_string()
                };
                format!("Running: {}", short_cmd)
            }
            _ => format!("Executing {}", name),
        }
    }

    /// Track file operations for the result
    fn track_file_operation(&mut self, tool_name: &str, input: &serde_json::Value) {
        let path = match tool_name {
            "read_file" | "list_file" => input.get("path").and_then(|v| v.as_str()),
            "write_file" => input.get("path").and_then(|v| v.as_str()),
            "edit_file" => input.get("file_path").and_then(|v| v.as_str()),
            _ => None,
        };

        if let Some(p) = path {
            match tool_name {
                "read_file" | "list_file" => {
                    if !self.files_read.contains(&p.to_string()) {
                        self.files_read.push(p.to_string());
                    }
                }
                "write_file" | "edit_file" => {
                    if !self.files_modified.contains(&p.to_string()) {
                        self.files_modified.push(p.to_string());
                    }
                }
                _ => {}
            }
        }
    }
}
