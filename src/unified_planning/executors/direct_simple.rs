//! Simplified Direct Executor
//!
//! Executes plan steps inline in the current Safe-Coder session.
//! This is a simplified version that focuses on the smart fallback optimization.

use anyhow::Result;
use async_trait::async_trait;

use crate::llm::{ContentBlock, Message, ToolDefinition};
use crate::tools::ToolContext;
use crate::unified_planning::{
    ExecutorContext, PlanExecutor, StepResult, StepResultBuilder, StepTimer, UnifiedPlan,
    UnifiedStep,
};

/// Direct executor - runs steps inline in the session
///
/// This executor is used for simple tasks that don't need parallelism
/// or isolation. Steps are executed sequentially using the session's
/// tool registry.
pub struct DirectExecutor {
    /// Enable logging for debugging
    debug_enabled: bool,
}

impl DirectExecutor {
    /// Create a new direct executor
    pub fn new() -> Self {
        Self {
            debug_enabled: false,
        }
    }
    
    /// Create with debug logging enabled
    pub fn with_debug(debug: bool) -> Self {
        Self {
            debug_enabled: debug,
        }
    }
}

impl Default for DirectExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlanExecutor for DirectExecutor {
    fn name(&self) -> &str {
        "direct"
    }

    fn supports_parallel(&self) -> bool {
        false // Direct execution is always sequential
    }

    fn supports_batching(&self) -> bool {
        false // Simplified version doesn't batch
    }

    fn max_concurrency(&self) -> usize {
        1
    }

    async fn execute_step(
        &self,
        step: &UnifiedStep,
        group_id: &str,
        ctx: &ExecutorContext,
    ) -> Result<StepResult> {
        let timer = StepTimer::start();

        // Emit step started event
        ctx.emit_step_started(group_id, step);

        ctx.emit_step_progress(&step.id, "Sending instructions to LLM...");

        // Build context message with step instructions
        let context_msg = if !step.relevant_files.is_empty() {
            format!(
                "Execute this step:\n\n{}\n\nInstructions:\n{}\n\nRelevant files: {}",
                step.description,
                step.instructions,
                step.relevant_files.join(", ")
            )
        } else {
            format!(
                "Execute this step:\n\n{}\n\nInstructions:\n{}",
                step.description, step.instructions
            )
        };

        // Send to LLM with available tools
        let messages = vec![Message::user(context_msg)];
        let tool_schemas = ctx.tool_registry.get_tools_schema();

        // Convert to ToolDefinition format
        let tools: Vec<ToolDefinition> = tool_schemas
            .iter()
            .map(|schema| ToolDefinition {
                name: schema["name"].as_str().unwrap_or_default().to_string(),
                description: schema["description"]
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                input_schema: schema["input_schema"].clone(),
            })
            .collect();

        let response = ctx
            .llm_client
            .send_message(&messages, &tools)
            .await
            .map_err(|e| anyhow::anyhow!("LLM request failed: {}", e))?;

        let mut output = String::new();
        let mut files_modified = Vec::new();
        let mut had_error = false;
        let mut error_message = None;

        // Extract text content and tool uses
        let mut tool_uses = Vec::new();
        for block in &response.message.content {
            match block {
                ContentBlock::Text { text } => {
                    output.push_str(text);
                    output.push('\n');
                }
                ContentBlock::ToolUse { id, name, input } => {
                    tool_uses.push((id.clone(), name.clone(), input.clone()));
                }
                _ => {}
            }
        }

        // Execute tool calls
        if !tool_uses.is_empty() {
            ctx.emit_step_progress(&step.id, &format!("Executing {} tools...", tool_uses.len()));

            for (call_id, tool_name, tool_input) in tool_uses {
                ctx.emit_step_progress(&step.id, &format!("Using tool: {}", tool_name));

                if let Some(tool) = ctx.tool_registry.get_tool(&tool_name) {
                    // Create tool context
                    let tool_config = Default::default(); // Use default config for now
                    let tool_context = ToolContext::new(&ctx.project_path, &tool_config);

                    match tool.execute(tool_input, &tool_context).await {
                        Ok(result) => {
                            output.push_str(&format!("\n[{}]: {}\n", tool_name, result));
                            // Note: We can't easily track files_modified from the string result
                            // This is a limitation of the current tool interface
                        }
                        Err(e) => {
                            let error_msg = format!("Tool {} failed: {}", tool_name, e);
                            output.push_str(&format!("\nError: {}\n", error_msg));
                            had_error = true;
                            if error_message.is_none() {
                                error_message = Some(error_msg);
                            }
                        }
                    }
                } else {
                    let error_msg = format!("Tool {} not found", tool_name);
                    output.push_str(&format!("\nError: {}\n", error_msg));
                    had_error = true;
                    if error_message.is_none() {
                        error_message = Some(error_msg);
                    }
                }
            }
        }

        let duration = timer.elapsed_ms();

        let result = if had_error {
            if let Some(error) = error_message {
                StepResultBuilder::failure()
                    .with_output(output)
                    .with_error(error)
                    .with_duration(duration)
                    .build()
            } else {
                StepResultBuilder::failure()
                    .with_output(output)
                    .with_duration(duration)
                    .build()
            }
        } else {
            StepResultBuilder::success()
                .with_output(output)
                .with_files(files_modified)
                .with_duration(duration)
                .build()
        };

        ctx.emit_step_completed(&step.id, &result);

        Ok(result)
    }
}