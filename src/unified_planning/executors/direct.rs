//! Direct Executor
//!
//! Executes plan steps inline in the current Safe-Coder session.
//! This is the simplest executor - sequential execution with no parallelism.

use anyhow::Result;
use async_trait::async_trait;

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

        // TODO: Integrate with session tool execution
        // For now, we just return the instructions as output
        // In the full implementation, this would:
        // 1. Send the step instructions to the LLM
        // 2. Execute any tool calls the LLM makes
        // 3. Collect the results

        ctx.emit_step_progress(&step.id, "Executing step...");

        // Placeholder execution
        let result = StepResultBuilder::success()
            .with_output(format!(
                "Executed: {}\nInstructions: {}",
                step.description, step.instructions
            ))
            .with_duration(timer.elapsed_ms())
            .with_files(step.relevant_files.clone())
            .build();

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
        let executor = DirectExecutor::new();
        let (tx, mut rx) = mpsc::unbounded_channel::<PlanEvent>();

        let ctx = ExecutorContext::new(
            PathBuf::from("/tmp"),
            Arc::new(Config::default()),
            tx,
            "plan-1".to_string(),
            ExecutionMode::Direct,
        );

        let step = UnifiedStep::new("step-1", "Test step").with_instructions("Do something");

        let result = executor.execute_step(&step, "group-1", &ctx).await.unwrap();

        assert!(result.success);
        assert!(result.output.contains("Test step"));

        // Check events were emitted
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }

        assert!(events.len() >= 2); // At least started and completed
    }
}
