//! Subagent Plan Executor
//!
//! Executes plan steps using internal specialized subagents.
//! Supports parallel execution within the same process.

use anyhow::Result;
use async_trait::async_trait;
use futures::future::join_all;

use crate::unified_planning::{
    ExecutorContext, PlanExecutor, StepExecutor, StepResult, StepResultBuilder, StepTimer,
    SubagentKind, UnifiedPlan, UnifiedStep,
};

/// Subagent executor - delegates to internal specialized agents
///
/// This executor spawns subagents for focused tasks. Multiple subagents
/// can run in parallel within the same process, sharing context.
pub struct SubagentPlanExecutor {
    /// Maximum number of concurrent subagents
    max_concurrent: usize,
}

impl SubagentPlanExecutor {
    /// Create a new subagent executor
    pub fn new() -> Self {
        Self { max_concurrent: 3 }
    }

    /// Create with custom concurrency limit
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }

    /// Get the subagent kind for a step
    fn get_subagent_kind(&self, step: &UnifiedStep) -> SubagentKind {
        match &step.suggested_executor {
            StepExecutor::Subagent { kind } => *kind,
            _ => SubagentKind::Custom, // Default to custom if not a subagent step
        }
    }

    /// Execute a single step with a subagent
    async fn execute_with_subagent(
        &self,
        step: &UnifiedStep,
        group_id: &str,
        ctx: &ExecutorContext,
        kind: SubagentKind,
    ) -> Result<StepResult> {
        let timer = StepTimer::start();

        ctx.emit_step_started(group_id, step);
        ctx.emit_step_progress(&step.id, &format!("Starting {} subagent...", kind));

        // TODO: Integrate with actual SubagentExecutor from src/subagent/
        // This would:
        // 1. Create SubagentScope from step instructions
        // 2. Spawn SubagentExecutor with appropriate kind
        // 3. Run the subagent and collect results

        ctx.emit_step_progress(&step.id, &format!("{} analyzing task...", kind));

        // Simulate some work
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Placeholder result
        let result = StepResultBuilder::success()
            .with_output(format!(
                "[{} subagent] Executed: {}\nInstructions: {}",
                kind, step.description, step.instructions
            ))
            .with_duration(timer.elapsed_ms())
            .with_files(step.relevant_files.clone())
            .build();

        ctx.emit_step_completed(&step.id, &result);

        Ok(result)
    }
}

impl Default for SubagentPlanExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlanExecutor for SubagentPlanExecutor {
    fn name(&self) -> &str {
        "subagent"
    }

    fn supports_parallel(&self) -> bool {
        true // Subagents can run in parallel
    }

    fn max_concurrency(&self) -> usize {
        self.max_concurrent
    }

    async fn execute_step(
        &self,
        step: &UnifiedStep,
        group_id: &str,
        ctx: &ExecutorContext,
    ) -> Result<StepResult> {
        let kind = self.get_subagent_kind(step);
        self.execute_with_subagent(step, group_id, ctx, kind).await
    }

    async fn execute_steps(
        &self,
        steps: &[UnifiedStep],
        group_id: &str,
        ctx: &ExecutorContext,
    ) -> Vec<Result<StepResult>> {
        // Execute steps in parallel, respecting max concurrency
        let chunks: Vec<_> = steps.chunks(self.max_concurrent).collect();
        let mut all_results = Vec::with_capacity(steps.len());

        for chunk in chunks {
            let futures: Vec<_> = chunk
                .iter()
                .map(|step| self.execute_step(step, group_id, ctx))
                .collect();

            let chunk_results = join_all(futures).await;
            all_results.extend(chunk_results);
        }

        all_results
    }

    async fn prepare(&self, _plan: &UnifiedPlan, ctx: &ExecutorContext) -> Result<()> {
        // Log preparation
        tracing::debug!(
            "Preparing subagent executor for plan {} with max {} concurrent",
            ctx.plan_id,
            self.max_concurrent
        );
        Ok(())
    }

    async fn finalize(&self, _plan: &UnifiedPlan, _ctx: &ExecutorContext) -> Result<()> {
        // No special finalization needed for subagents
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
    async fn test_subagent_executor_basic() {
        let executor = SubagentPlanExecutor::new();

        assert_eq!(executor.name(), "subagent");
        assert!(executor.supports_parallel());
        assert_eq!(executor.max_concurrency(), 3);
    }

    #[tokio::test]
    async fn test_subagent_executor_custom_concurrency() {
        let executor = SubagentPlanExecutor::new().with_max_concurrent(5);
        assert_eq!(executor.max_concurrency(), 5);
    }

    #[tokio::test]
    async fn test_subagent_executor_execute_step() {
        let executor = SubagentPlanExecutor::new();
        let (tx, _rx) = mpsc::unbounded_channel::<PlanEvent>();

        let ctx = ExecutorContext::new(
            PathBuf::from("/tmp"),
            Arc::new(Config::default()),
            tx,
            "plan-1".to_string(),
            ExecutionMode::Subagent,
        );

        let step = UnifiedStep::new("step-1", "Test step")
            .with_instructions("Do something")
            .with_executor(StepExecutor::Subagent {
                kind: SubagentKind::Tester,
            });

        let result = executor.execute_step(&step, "group-1", &ctx).await.unwrap();

        assert!(result.success);
        assert!(result.output.contains("tester subagent"));
    }

    #[tokio::test]
    async fn test_subagent_executor_parallel_execution() {
        let executor = SubagentPlanExecutor::new();
        let (tx, _rx) = mpsc::unbounded_channel::<PlanEvent>();

        let ctx = ExecutorContext::new(
            PathBuf::from("/tmp"),
            Arc::new(Config::default()),
            tx,
            "plan-1".to_string(),
            ExecutionMode::Subagent,
        );

        let steps = vec![
            UnifiedStep::new("step-1", "Step 1").with_executor(StepExecutor::Subagent {
                kind: SubagentKind::Tester,
            }),
            UnifiedStep::new("step-2", "Step 2").with_executor(StepExecutor::Subagent {
                kind: SubagentKind::Refactorer,
            }),
        ];

        let results = executor.execute_steps(&steps, "group-1", &ctx).await;

        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.is_ok()));
    }
}
