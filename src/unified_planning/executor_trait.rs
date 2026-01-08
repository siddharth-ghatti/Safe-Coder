//! Unified Executor Interface
//!
//! Defines the `PlanExecutor` trait that all executors must implement,
//! ensuring standardized invocation across all execution modes.

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::Config;
use crate::llm::LlmClient;
use crate::tools::ToolRegistry;

use super::types::{ExecutionMode, PlanEvent, StepResult, UnifiedPlan, UnifiedStep};

/// Context passed to every executor
///
/// Contains all the information an executor needs to execute steps.
pub struct ExecutorContext {
    /// Path to the project root
    pub project_path: PathBuf,
    /// Application configuration
    pub config: Arc<Config>,
    /// Channel to send execution events
    pub event_tx: UnboundedSender<PlanEvent>,
    /// The plan ID being executed
    pub plan_id: String,
    /// The execution mode
    pub execution_mode: ExecutionMode,
    /// LLM client for AI interactions
    pub llm_client: Arc<dyn LlmClient>,
    /// Tool registry for executing tools
    pub tool_registry: Arc<ToolRegistry>,
}

impl ExecutorContext {
    /// Create a new executor context
    pub fn new(
        project_path: PathBuf,
        config: Arc<Config>,
        event_tx: UnboundedSender<PlanEvent>,
        plan_id: String,
        execution_mode: ExecutionMode,
        llm_client: Arc<dyn LlmClient>,
        tool_registry: Arc<ToolRegistry>,
    ) -> Self {
        Self {
            project_path,
            config,
            event_tx,
            plan_id,
            execution_mode,
            llm_client,
            tool_registry,
        }
    }

    /// Emit a plan event
    pub fn emit(&self, event: PlanEvent) {
        let _ = self.event_tx.send(event);
    }

    /// Emit a step started event
    pub fn emit_step_started(&self, group_id: &str, step: &UnifiedStep) {
        self.emit(PlanEvent::StepStarted {
            plan_id: self.plan_id.clone(),
            group_id: group_id.to_string(),
            step_id: step.id.clone(),
            description: step.active_description.clone(),
        });
    }

    /// Emit a step progress event
    pub fn emit_step_progress(&self, step_id: &str, message: &str) {
        self.emit(PlanEvent::StepProgress {
            plan_id: self.plan_id.clone(),
            step_id: step_id.to_string(),
            message: message.to_string(),
        });
    }

    /// Emit a step completed event
    pub fn emit_step_completed(&self, step_id: &str, result: &StepResult) {
        self.emit(PlanEvent::StepCompleted {
            plan_id: self.plan_id.clone(),
            step_id: step_id.to_string(),
            success: result.success,
            duration_ms: result.duration_ms,
        });
    }
}

/// The unified executor interface - ALL executors implement this
///
/// This trait defines the contract for executing plan steps. Each execution mode
/// (Direct, Subagent, Orchestration) implements this trait differently.
#[async_trait]
pub trait PlanExecutor: Send + Sync {
    /// Get the executor's name (for logging/display)
    fn name(&self) -> &str;

    /// Check if this executor supports parallel step execution
    fn supports_parallel(&self) -> bool;

    /// Get maximum concurrency (only relevant if supports_parallel = true)
    fn max_concurrency(&self) -> usize {
        1
    }

    /// Execute a single step
    ///
    /// This is the core execution method. Each executor implements this
    /// according to its execution strategy.
    async fn execute_step(
        &self,
        step: &UnifiedStep,
        group_id: &str,
        ctx: &ExecutorContext,
    ) -> Result<StepResult>;

    /// Check if this executor can batch multiple steps into a single request
    fn supports_batching(&self) -> bool {
        false
    }

    /// Execute multiple steps with optional batching
    ///
    /// Default implementation executes sequentially. Parallel-capable executors
    /// should override this to use concurrent execution. Batching-capable executors
    /// should override this to group compatible steps.
    async fn execute_steps(
        &self,
        steps: &[UnifiedStep],
        group_id: &str,
        ctx: &ExecutorContext,
    ) -> Vec<Result<StepResult>> {
        let mut results = Vec::with_capacity(steps.len());
        for step in steps {
            results.push(self.execute_step(step, group_id, ctx).await);
        }
        results
    }

    /// Called before execution starts
    ///
    /// Use this for setup like creating workspaces, initializing resources, etc.
    async fn prepare(&self, _plan: &UnifiedPlan, _ctx: &ExecutorContext) -> Result<()> {
        Ok(())
    }

    /// Called after execution completes
    ///
    /// Use this for cleanup, merging results, etc.
    async fn finalize(&self, _plan: &UnifiedPlan, _ctx: &ExecutorContext) -> Result<()> {
        Ok(())
    }

    /// Called when execution is cancelled
    ///
    /// Use this for emergency cleanup.
    async fn cancel(&self, _plan: &UnifiedPlan, _ctx: &ExecutorContext) -> Result<()> {
        Ok(())
    }
}

/// Registry of available executors
///
/// Maps execution modes to their executor implementations.
pub struct ExecutorRegistry {
    executors: HashMap<ExecutionMode, Arc<dyn PlanExecutor>>,
}

impl ExecutorRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            executors: HashMap::new(),
        }
    }

    /// Register an executor for a mode
    pub fn register(&mut self, mode: ExecutionMode, executor: Arc<dyn PlanExecutor>) {
        self.executors.insert(mode, executor);
    }

    /// Get the executor for a mode
    pub fn get(&self, mode: ExecutionMode) -> Option<Arc<dyn PlanExecutor>> {
        self.executors.get(&mode).cloned()
    }

    /// Check if a mode has an executor registered
    pub fn has(&self, mode: ExecutionMode) -> bool {
        self.executors.contains_key(&mode)
    }

    /// Get all registered modes
    pub fn modes(&self) -> Vec<ExecutionMode> {
        self.executors.keys().copied().collect()
    }
}

impl Default for ExecutorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper struct for building step results
pub struct StepResultBuilder {
    success: bool,
    output: String,
    error: Option<String>,
    duration_ms: u64,
    files_modified: Vec<String>,
}

impl StepResultBuilder {
    /// Create a new builder for a successful result
    pub fn success() -> Self {
        Self {
            success: true,
            output: String::new(),
            error: None,
            duration_ms: 0,
            files_modified: Vec::new(),
        }
    }

    /// Create a new builder for a failed result
    pub fn failure() -> Self {
        Self {
            success: false,
            output: String::new(),
            error: None,
            duration_ms: 0,
            files_modified: Vec::new(),
        }
    }

    /// Set the output
    pub fn with_output(mut self, output: impl Into<String>) -> Self {
        self.output = output.into();
        self
    }

    /// Set the error message
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self.success = false;
        self
    }

    /// Set the duration
    pub fn with_duration(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    /// Add a modified file
    pub fn add_file(mut self, file: impl Into<String>) -> Self {
        self.files_modified.push(file.into());
        self
    }

    /// Set modified files
    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.files_modified = files;
        self
    }

    /// Build the result
    pub fn build(self) -> StepResult {
        StepResult {
            success: self.success,
            output: self.output,
            error: self.error,
            duration_ms: self.duration_ms,
            files_modified: self.files_modified,
        }
    }
}

/// Utility for measuring step execution time
pub struct StepTimer {
    start: std::time::Instant,
}

impl StepTimer {
    /// Start a new timer
    pub fn start() -> Self {
        Self {
            start: std::time::Instant::now(),
        }
    }

    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> u64 {
        self.start.elapsed().as_millis() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[test]
    fn test_step_result_builder_success() {
        let result = StepResultBuilder::success()
            .with_output("Done!")
            .with_duration(100)
            .add_file("src/main.rs")
            .build();

        assert!(result.success);
        assert_eq!(result.output, "Done!");
        assert_eq!(result.duration_ms, 100);
        assert_eq!(result.files_modified, vec!["src/main.rs"]);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_step_result_builder_failure() {
        let result = StepResultBuilder::failure()
            .with_error("Something went wrong")
            .with_duration(50)
            .build();

        assert!(!result.success);
        assert_eq!(result.error, Some("Something went wrong".to_string()));
        assert_eq!(result.duration_ms, 50);
    }

    #[test]
    fn test_executor_registry() {
        let registry = ExecutorRegistry::new();
        assert!(!registry.has(ExecutionMode::Direct));
        assert!(registry.get(ExecutionMode::Direct).is_none());
    }

    #[tokio::test]
    async fn test_executor_context_emit() {
        use crate::llm::create_client;
        use crate::tools::ToolRegistry;

        let (tx, mut rx) = mpsc::unbounded_channel();
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

        ctx.emit(PlanEvent::PlanStarted {
            plan_id: "plan-1".to_string(),
        });

        let event = rx.recv().await.unwrap();
        match event {
            PlanEvent::PlanStarted { plan_id } => assert_eq!(plan_id, "plan-1"),
            _ => panic!("Unexpected event"),
        }
    }
}
