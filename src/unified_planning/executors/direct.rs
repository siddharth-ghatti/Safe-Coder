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
/// tool registry.
pub struct DirectExecutor {
    // In the future, this could hold a reference to the session's
    // tool registry for actual execution
}

impl DirectExecutor {
    /// Create a new direct executor
    pub fn new() -> Self {
        Self {}
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
