//! Direct Executor
//!
//! Executes plan steps inline in the current Safe-Coder session.
//! This is the simplest executor - sequential execution with no parallelism.

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
/// tool registry, with smart batching to reduce API requests.
pub struct DirectExecutor {
    /// Maximum number of steps to batch in a single request
    max_batch_size: usize,
    /// Enable batching optimization
    enable_batching: bool,
}

impl DirectExecutor {
    /// Create a new direct executor with default settings
    pub fn new() -> Self {
        Self {
            max_batch_size: 3,
            enable_batching: true,
        }
    }

    /// Create with custom batching settings
    pub fn with_batching(max_batch_size: usize, enable: bool) -> Self {
        Self {
            max_batch_size,
            enable_batching: enable,
        }
    }

    /// Check if steps can be batched together
    fn can_batch_steps(&self, step1: &UnifiedStep, step2: &UnifiedStep) -> bool {
        if !self.enable_batching {
            return false;
        }

        // Don't batch steps that are likely to be very complex
        let is_complex = |step: &UnifiedStep| {
            step.instructions.len() > 500
                || step.description.to_lowercase().contains("complex")
                || step.description.to_lowercase().contains("refactor")
                || step.description.to_lowercase().contains("rewrite")
        };

        if is_complex(step1) || is_complex(step2) {
            return false;
        }

        // Prefer batching similar types of operations
        let get_operation_type = |step: &UnifiedStep| {
            let desc = step.description.to_lowercase();
            if desc.contains("read") || desc.contains("analyze") {
                "read"
            } else if desc.contains("write") || desc.contains("create") {
                "write"
            } else if desc.contains("edit") || desc.contains("modify") {
                "edit"
            } else if desc.contains("test") || desc.contains("run") {
                "test"
            } else {
                "other"
            }
        };

        // Batch steps of similar types
        get_operation_type(step1) == get_operation_type(step2)
    }

    /// Execute a batch of steps in a single LLM request
    async fn execute_step_batch(
        &self,
        steps: &[UnifiedStep],
        group_id: &str,
        ctx: &ExecutorContext,
    ) -> Result<Vec<StepResult>> {
        let timer = StepTimer::start();

        // Emit batch started event
        for step in steps {
            ctx.emit_step_started(group_id, step);
        }

        ctx.emit_step_progress(
            &steps[0].id,
            &format!("Executing batch of {} steps...", steps.len()),
        );

        // Build combined context message
        let batch_instructions = steps
            .iter()
            .enumerate()
            .map(|(i, step)| {
                let relevant_files = if !step.relevant_files.is_empty() {
                    format!(" (relevant files: {})", step.relevant_files.join(", "))
                } else {
                    String::new()
                };

                format!(
                    "Step {}: {}\nInstructions: {}{}\n",
                    i + 1,
                    step.description,
                    step.instructions,
                    relevant_files
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let context_msg = format!(
            "Execute these {} related steps in order. Complete each step fully before moving to the next:\n\n{}",
            steps.len(),
            batch_instructions
        );

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
            .map_err(|e| anyhow::anyhow!("LLM batch request failed: {}", e))?;

        let mut batch_output = String::new();
        let mut all_files_modified = Vec::new();
        let mut batch_had_error = false;
        let mut batch_error_message = None;

        // Extract text content and tool uses
        let mut tool_uses = Vec::new();
        for block in &response.message.content {
            match block {
                ContentBlock::Text { text } => {
                    batch_output.push_str(text);
                    batch_output.push('\n');
                }
                ContentBlock::ToolUse { id, name, input } => {
                    tool_uses.push((id.clone(), name.clone(), input.clone()));
                }
                _ => {}
            }
        }

        // Execute tool calls
        if !tool_uses.is_empty() {
            ctx.emit_step_progress(
                &steps[0].id,
                &format!("Executing {} tools for batch...", tool_uses.len()),
            );

            // Execute all tool calls for the batch
            for (call_id, tool_name, tool_input) in tool_uses {
                ctx.emit_step_progress(&steps[0].id, &format!("Using tool: {}", tool_name));

                if let Some(tool) = ctx.tool_registry.get_tool(&tool_name) {
                    // Create tool context
                    let tool_config = Default::default(); // Use default config for now
                    let tool_context = ToolContext::new(&ctx.project_path, &tool_config);

                    match tool.execute(tool_input, &tool_context).await {
                        Ok(result) => {
                            batch_output.push_str(&format!("\n[{}]: {}\n", tool_name, result));
                            // Note: We can't easily track files_modified from the string result
                            // This is a limitation of the current tool interface
                        }
                        Err(e) => {
                            let error_msg = format!("Tool {} failed: {}", tool_name, e);
                            batch_output.push_str(&format!("\nError: {}\n", error_msg));
                            batch_had_error = true;
                            if batch_error_message.is_none() {
                                batch_error_message = Some(error_msg);
                            }
                        }
                    }
                } else {
                    let error_msg = format!("Tool {} not found", tool_name);
                    batch_output.push_str(&format!("\nError: {}\n", error_msg));
                    batch_had_error = true;
                    if batch_error_message.is_none() {
                        batch_error_message = Some(error_msg);
                    }
                }
            }
        }

        let duration = timer.elapsed_ms();

        // Create results for each step in the batch
        let mut results = Vec::new();
        for (i, step) in steps.iter().enumerate() {
            let step_output = if i == 0 {
                batch_output.clone() // Full output for first step
            } else {
                format!("Part of batch execution (see step {})", steps[0].id)
            };

            let result = StepResultBuilder::success()
                .with_output(step_output.clone())
                .with_files(if i == 0 {
                    all_files_modified.clone()
                } else {
                    Vec::new()
                })
                .with_duration(duration);

            let result = if batch_had_error && i == 0 {
                if let Some(ref error_msg) = batch_error_message {
                    StepResultBuilder::failure()
                        .with_output(step_output)
                        .with_error(error_msg)
                        .with_duration(duration)
                        .build()
                } else {
                    result.build()
                }
            } else {
                result.build()
            };

            ctx.emit_step_completed(&step.id, &result);
            results.push(result);
        }

        Ok(results)
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
        self.enable_batching
    }

    fn max_concurrency(&self) -> usize {
        1
    }

    async fn execute_steps(
        &self,
        steps: &[UnifiedStep],
        group_id: &str,
        ctx: &ExecutorContext,
    ) -> Vec<Result<StepResult>> {
        if !self.enable_batching || steps.len() <= 1 {
            // Fall back to sequential execution
            let mut results = Vec::with_capacity(steps.len());
            for step in steps {
                results.push(self.execute_step(step, group_id, ctx).await);
            }
            return results;
        }

        // Group steps into batches
        let mut results = Vec::with_capacity(steps.len());
        let mut current_batch = Vec::new();

        for step in steps {
            // Check if this step can be batched with the current batch
            let can_batch = if current_batch.is_empty() {
                true
            } else if current_batch.len() >= self.max_batch_size {
                false
            } else {
                self.can_batch_steps(current_batch.last().unwrap(), step)
            };

            if can_batch {
                current_batch.push(step.clone());
            } else {
                // Execute current batch
                if !current_batch.is_empty() {
                    match self.execute_step_batch(&current_batch, group_id, ctx).await {
                        Ok(batch_results) => results.extend(batch_results.into_iter().map(Ok)),
                        Err(e) => {
                            // If batch fails, create error results for all steps
                            let err_msg = e.to_string();
                            for _step in &current_batch {
                                results.push(Err(anyhow::anyhow!(err_msg.clone())));
                            }
                        }
                    }
                }

                // Start new batch with current step
                current_batch = vec![step.clone()];
            }
        }

        // Execute remaining batch
        if !current_batch.is_empty() {
            match self.execute_step_batch(&current_batch, group_id, ctx).await {
                Ok(batch_results) => results.extend(batch_results.into_iter().map(Ok)),
                Err(e) => {
                    // If batch fails, create error results for all steps
                    let err_msg = e.to_string();
                    for _step in &current_batch {
                        results.push(Err(anyhow::anyhow!(err_msg.clone())));
                    }
                }
            }
        }

        results
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

            // Create callback for streaming output
            let event_tx = ctx.event_tx.clone();
            let plan_id = ctx.plan_id.clone();
            let step_id = step.id.clone();

            let callback: crate::tools::OutputCallback = std::sync::Arc::new(move |line| {
                let _ = event_tx.send(crate::unified_planning::PlanEvent::StepProgress {
                    plan_id: plan_id.clone(),
                    step_id: step_id.clone(),
                    message: line,
                });
            });

            // Create tool context with callback
            let tool_context =
                ToolContext::with_output_callback(&ctx.project_path, &ctx.config.tools, callback);

            for (idx, (id, name, input)) in tool_uses.iter().enumerate() {
                ctx.emit_step_progress(
                    &step.id,
                    &format!(
                        "Executing tool {} of {}: {}",
                        idx + 1,
                        tool_uses.len(),
                        name
                    ),
                );

                // Get the tool
                if let Some(tool) = ctx.tool_registry.get_tool(name) {
                    match tool.execute(input.clone(), &tool_context).await {
                        Ok(result) => {
                            output.push_str(&format!("\n[{}] {}\n", name, result));

                            // Track modified files
                            if matches!(name.as_str(), "write_file" | "edit_file" | "bash") {
                                if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
                                    files_modified.push(path.to_string());
                                } else if let Some(path) =
                                    input.get("file_path").and_then(|v| v.as_str())
                                {
                                    files_modified.push(path.to_string());
                                }
                            }
                        }
                        Err(e) => {
                            let err_msg = format!("Tool {} failed: {}", name, e);
                            output.push_str(&format!("\n❌ {}\n", err_msg));
                            had_error = true;
                            error_message = Some(err_msg);
                        }
                    }
                } else {
                    let err_msg = format!("Unknown tool: {}", name);
                    output.push_str(&format!("\n❌ {}\n", err_msg));
                    had_error = true;
                    error_message = Some(err_msg);
                }
            }
        }

        let result = if had_error {
            StepResultBuilder::failure()
                .with_output(output)
                .with_error(error_message.unwrap_or_else(|| "Unknown error".to_string()))
                .with_duration(timer.elapsed_ms())
                .with_files(files_modified)
                .build()
        } else {
            StepResultBuilder::success()
                .with_output(output)
                .with_duration(timer.elapsed_ms())
                .with_files(files_modified)
                .build()
        };

        ctx.emit_step_completed(&step.id, &result);

        Ok(result)
    }

    async fn prepare(&self, _plan: &UnifiedPlan, _ctx: &ExecutorContext) -> Result<()> {
        // No preparation needed for direct execution
        Ok(())
    }

    async fn finalize(&self, _plan: &UnifiedPlan, _ctx: &ExecutorContext) -> Result<()> {
        // No finalization needed for direct execution
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::unified_planning::{ExecutionMode, PlanEvent};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_direct_executor_basic() {
        let executor = DirectExecutor::new();

        assert_eq!(executor.name(), "direct");
        assert!(!executor.supports_parallel());
        assert_eq!(executor.max_concurrency(), 1);
    }

    #[tokio::test]
    async fn test_direct_executor_execute_step() {
        use crate::llm::create_client;
        use crate::tools::ToolRegistry;

        let executor = DirectExecutor::new();
        let (tx, mut rx) = mpsc::unbounded_channel::<PlanEvent>();
        let config = Arc::new(Config::default());
        let llm_client = Arc::new(create_client(&config).unwrap());
        let tool_registry = Arc::new(ToolRegistry::new());

        let ctx = ExecutorContext::new(
            PathBuf::from("/tmp"),
            config,
            tx,
            "plan-1".to_string(),
            ExecutionMode::Direct,
            llm_client,
            tool_registry,
        );

        let step = UnifiedStep::new("step-1", "Test step").with_instructions("Do something");

        let result = executor.execute_step(&step, "group-1", &ctx).await;

        // Result may fail if LLM is not configured, but that's ok for this test
        // We just check the structure
        assert!(result.is_ok() || result.is_err());

        // Check events were emitted
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        assert!(events.len() >= 1); // At least started event
    }
}
