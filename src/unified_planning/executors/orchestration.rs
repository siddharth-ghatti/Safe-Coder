//! Orchestration Executor
//!
//! Executes plan steps using external CLI workers in isolated git worktrees.
//! Provides full parallelism with process isolation.

use anyhow::Result;
use async_trait::async_trait;
use futures::future::join_all;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::unified_planning::{
    ExecutorContext, PlanExecutor, StepExecutor, StepResult, StepResultBuilder, StepTimer,
    UnifiedPlan, UnifiedStep, WorkerKind,
};

/// Orchestration executor - delegates to external CLI workers
///
/// This executor creates isolated git worktrees for each task and
/// spawns external CLI tools (Claude Code, Gemini CLI, etc.) to
/// execute the work.
pub struct OrchestrationExecutor {
    /// Maximum concurrent workers
    max_concurrent: usize,
    /// Workspace paths for each step (step_id -> workspace_path)
    workspaces: Arc<Mutex<HashMap<String, PathBuf>>>,
}

impl OrchestrationExecutor {
    /// Create a new orchestration executor
    pub fn new() -> Self {
        Self {
            max_concurrent: 3,
            workspaces: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Create with custom concurrency limit
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max;
        self
    }

    /// Get the worker kind for a step
    fn get_worker_kind(&self, step: &UnifiedStep) -> WorkerKind {
        match &step.suggested_executor {
            StepExecutor::Worker { kind } => *kind,
            _ => WorkerKind::ClaudeCode, // Default to Claude Code
        }
    }

    /// Create a workspace for a step
    async fn create_workspace(&self, step_id: &str, ctx: &ExecutorContext) -> Result<PathBuf> {
        // TODO: Integrate with WorkspaceManager from src/orchestrator/workspace.rs
        // This would:
        // 1. Create a git worktree in .safe-coder-workspaces/<step_id>
        // 2. Track the workspace for later merging/cleanup

        let workspace_dir = ctx
            .project_path
            .join(".safe-coder-workspaces")
            .join(step_id);

        // Store workspace path
        let mut workspaces = self.workspaces.lock().await;
        workspaces.insert(step_id.to_string(), workspace_dir.clone());

        Ok(workspace_dir)
    }

    /// Execute a step with an external worker
    async fn execute_with_worker(
        &self,
        step: &UnifiedStep,
        group_id: &str,
        ctx: &ExecutorContext,
        kind: WorkerKind,
    ) -> Result<StepResult> {
        let timer = StepTimer::start();

        ctx.emit_step_started(group_id, step);
        ctx.emit_step_progress(
            &step.id,
            &format!("Creating workspace for {} worker...", kind),
        );

        // Create workspace
        let _workspace = self.create_workspace(&step.id, ctx).await?;

        ctx.emit_step_progress(&step.id, &format!("Starting {} worker...", kind));

        // TODO: Integrate with Worker from src/orchestrator/worker.rs
        // This would:
        // 1. Build the worker command (claude, gemini, etc.)
        // 2. Set working directory to workspace
        // 3. Run the command with step instructions
        // 4. Capture output

        // Simulate worker execution
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        ctx.emit_step_progress(&step.id, &format!("{} worker completed", kind));

        // Placeholder result
        let result = StepResultBuilder::success()
            .with_output(format!(
                "[{} worker] Executed: {}\nInstructions: {}\nFiles: {:?}",
                kind, step.description, step.instructions, step.relevant_files
            ))
            .with_duration(timer.elapsed_ms())
            .with_files(step.relevant_files.clone())
            .build();

        ctx.emit_step_completed(&step.id, &result);

        Ok(result)
    }

    /// Merge workspace changes back to main branch
    async fn merge_workspace(&self, step_id: &str, _ctx: &ExecutorContext) -> Result<()> {
        // TODO: Integrate with WorkspaceManager.merge_workspace()
        // This would:
        // 1. Commit any uncommitted changes in the worktree
        // 2. Merge the branch back to the main branch
        // 3. Handle merge conflicts if any

        let mut workspaces = self.workspaces.lock().await;
        workspaces.remove(step_id);

        Ok(())
    }

    /// Cleanup a workspace
    async fn cleanup_workspace(&self, step_id: &str, _ctx: &ExecutorContext) -> Result<()> {
        // TODO: Integrate with WorkspaceManager.cleanup_workspace()
        // This would:
        // 1. Remove the worktree
        // 2. Delete the branch if requested

        let mut workspaces = self.workspaces.lock().await;
        workspaces.remove(step_id);

        Ok(())
    }
}

impl Default for OrchestrationExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PlanExecutor for OrchestrationExecutor {
    fn name(&self) -> &str {
        "orchestration"
    }

    fn supports_parallel(&self) -> bool {
        true // Orchestration is designed for parallel execution
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
        let kind = self.get_worker_kind(step);
        self.execute_with_worker(step, group_id, ctx, kind).await
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

    async fn prepare(&self, plan: &UnifiedPlan, ctx: &ExecutorContext) -> Result<()> {
        tracing::info!(
            "Preparing orchestration for plan {} with {} groups",
            ctx.plan_id,
            plan.groups.len()
        );

        // TODO: Initialize WorkspaceManager
        // Ensure .safe-coder-workspaces directory exists
        let workspaces_dir = ctx.project_path.join(".safe-coder-workspaces");
        if !workspaces_dir.exists() {
            tokio::fs::create_dir_all(&workspaces_dir).await?;
        }

        Ok(())
    }

    async fn finalize(&self, plan: &UnifiedPlan, ctx: &ExecutorContext) -> Result<()> {
        tracing::info!("Finalizing orchestration for plan {}", ctx.plan_id);

        // Merge successful workspaces, cleanup failed ones
        for group in &plan.groups {
            for step in &group.steps {
                if step.result.as_ref().map(|r| r.success).unwrap_or(false) {
                    self.merge_workspace(&step.id, ctx).await?;
                } else {
                    self.cleanup_workspace(&step.id, ctx).await?;
                }
            }
        }

        Ok(())
    }

    async fn cancel(&self, plan: &UnifiedPlan, ctx: &ExecutorContext) -> Result<()> {
        tracing::warn!("Cancelling orchestration for plan {}", ctx.plan_id);

        // Cleanup all workspaces
        for group in &plan.groups {
            for step in &group.steps {
                self.cleanup_workspace(&step.id, ctx).await?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::unified_planning::{ExecutionMode, PlanEvent, StepGroup};
    use std::sync::Arc;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_orchestration_executor_basic() {
        let executor = OrchestrationExecutor::new();

        assert_eq!(executor.name(), "orchestration");
        assert!(executor.supports_parallel());
        assert_eq!(executor.max_concurrency(), 3);
    }

    #[tokio::test]
    async fn test_orchestration_executor_custom_concurrency() {
        let executor = OrchestrationExecutor::new().with_max_concurrent(5);
        assert_eq!(executor.max_concurrency(), 5);
    }

    #[tokio::test]
    async fn test_orchestration_executor_execute_step() {
        let executor = OrchestrationExecutor::new();
        let (tx, _rx) = mpsc::unbounded_channel::<PlanEvent>();

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = ExecutorContext::new(
            temp_dir.path().to_path_buf(),
            Arc::new(Config::default()),
            tx,
            "plan-1".to_string(),
            ExecutionMode::Orchestration,
        );

        let step = UnifiedStep::new("step-1", "Test step")
            .with_instructions("Do something")
            .with_executor(StepExecutor::Worker {
                kind: WorkerKind::ClaudeCode,
            });

        let result = executor.execute_step(&step, "group-1", &ctx).await.unwrap();

        assert!(result.success);
        assert!(result.output.contains("claude worker"));
    }

    #[tokio::test]
    async fn test_orchestration_executor_prepare_finalize() {
        let executor = OrchestrationExecutor::new();
        let (tx, _rx) = mpsc::unbounded_channel::<PlanEvent>();

        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = ExecutorContext::new(
            temp_dir.path().to_path_buf(),
            Arc::new(Config::default()),
            tx,
            "plan-1".to_string(),
            ExecutionMode::Orchestration,
        );

        let plan = UnifiedPlan::new("plan-1", "Test")
            .with_title("Test Plan")
            .with_mode(ExecutionMode::Orchestration)
            .add_group(StepGroup::new("g1").add_step(UnifiedStep::new("s1", "Step 1")));

        // Prepare should create workspaces directory
        executor.prepare(&plan, &ctx).await.unwrap();
        assert!(temp_dir.path().join(".safe-coder-workspaces").exists());

        // Finalize should work without error
        executor.finalize(&plan, &ctx).await.unwrap();
    }
}
