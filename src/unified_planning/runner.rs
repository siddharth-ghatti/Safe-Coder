//! Plan Runner
//!
//! Orchestrates the execution of a unified plan through its lifecycle:
//! 1. Get executor from registry based on execution mode
//! 2. Call executor.prepare() for setup
//! 3. Execute groups in order, with parallel steps within groups
//! 4. Call executor.finalize() for cleanup
//! 5. Emit events throughout for UI updates

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::config::Config;
use crate::llm::LlmClient;
use crate::tools::ToolRegistry;

use super::executor_trait::{ExecutorContext, ExecutorRegistry, PlanExecutor};
use super::types::{ExecutionMode, PlanEvent, PlanStatus, StepStatus, UnifiedPlan};

/// Handles the lifecycle of plan execution
pub struct PlanRunner {
    /// Project path
    project_path: PathBuf,
    /// Configuration
    config: Arc<Config>,
    /// Executor registry
    registry: Arc<ExecutorRegistry>,
    /// LLM client for AI interactions
    llm_client: Arc<dyn LlmClient>,
    /// Tool registry for executing tools
    tool_registry: Arc<ToolRegistry>,
    /// Whether to require user approval before execution
    requires_approval: bool,
    /// Approval callback (returns true if approved) - for sync approval
    approval_callback: Option<Box<dyn Fn(&UnifiedPlan) -> bool + Send + Sync>>,
    /// Async approval receiver - for TUI-based approval (using unbounded for cloneable sender)
    approval_rx: Option<UnboundedReceiver<bool>>,
}

impl PlanRunner {
    /// Create a new plan runner
    pub fn new(
        project_path: PathBuf,
        config: Arc<Config>,
        registry: Arc<ExecutorRegistry>,
        llm_client: Arc<dyn LlmClient>,
        tool_registry: Arc<ToolRegistry>,
    ) -> Self {
        Self {
            project_path,
            config,
            registry,
            llm_client,
            tool_registry,
            requires_approval: false,
            approval_callback: None,
            approval_rx: None,
        }
    }

    /// Set whether approval is required
    pub fn with_approval(mut self, requires: bool) -> Self {
        self.requires_approval = requires;
        self
    }

    /// Set an approval callback (sync version)
    pub fn with_approval_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&UnifiedPlan) -> bool + Send + Sync + 'static,
    {
        self.approval_callback = Some(Box::new(callback));
        self.requires_approval = true;
        self
    }

    /// Set async approval receiver (for TUI-based approval)
    pub fn with_async_approval(mut self, rx: UnboundedReceiver<bool>) -> Self {
        self.approval_rx = Some(rx);
        self.requires_approval = true;
        self
    }

    /// Execute a plan and return the event receiver
    ///
    /// execution happens in a background task, allowing events to be streamed immediately.
    /// Note: Takes `mut self` to allow taking the approval receiver.
    pub async fn execute(
        mut self,
        mut plan: UnifiedPlan,
    ) -> Result<(UnifiedPlan, UnboundedReceiver<PlanEvent>)> {
        // Create event channel
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        // Emit plan created event with full plan for UI display
        event_tx
            .send(PlanEvent::PlanCreated {
                plan_id: plan.id.clone(),
                title: plan.title.clone(),
                total_steps: plan.total_steps(),
                execution_mode: plan.execution_mode,
                plan: plan.clone(),
            })
            .ok();

        // Prepare for background execution
        // We execute on a clone so we can return the original plan handle immediately
        let mut plan_for_exec = plan.clone();

        // Capture dependencies for the background task
        let project_path = self.project_path.clone();
        let config = self.config.clone();
        let registry = self.registry.clone();
        let llm_client = self.llm_client.clone();
        let tool_registry = self.tool_registry.clone();
        let requires_approval = self.requires_approval;
        let approval_rx = self.approval_rx.take();

        // Spawn execution task (includes approval handling so events can flow immediately)
        tokio::spawn(async move {
            // Handle approval if required (inside spawned task so events flow)
            if requires_approval {
                plan_for_exec.status = PlanStatus::AwaitingApproval;
                event_tx
                    .send(PlanEvent::PlanAwaitingApproval {
                        plan_id: plan_for_exec.id.clone(),
                    })
                    .ok();

                // Wait for async approval
                let approved = if let Some(mut rx) = approval_rx {
                    match rx.recv().await {
                        Some(approved) => approved,
                        None => {
                            // Channel closed, treat as rejection
                            false
                        }
                    }
                } else {
                    // No async approval channel, auto-approve
                    true
                };

                if !approved {
                    event_tx
                        .send(PlanEvent::PlanRejected {
                            plan_id: plan_for_exec.id.clone(),
                            reason: "User rejected the plan".to_string(),
                        })
                        .ok();
                    plan_for_exec.status = PlanStatus::Cancelled;
                    return;
                }

                event_tx
                    .send(PlanEvent::PlanApproved {
                        plan_id: plan_for_exec.id.clone(),
                    })
                    .ok();
            }

            let result = Self::execute_plan_logic(
                &mut plan_for_exec,
                event_tx.clone(),
                project_path,
                config,
                registry,
                llm_client,
                tool_registry,
            )
            .await;

            if let Err(e) = result {
                tracing::error!("Plan execution failed: {}", e);
            }
        });

        Ok((plan, event_rx))
    }

    /// Static implementation of plan execution logic to be run in background
    async fn execute_plan_logic(
        plan: &mut UnifiedPlan,
        event_tx: UnboundedSender<PlanEvent>,
        project_path: PathBuf,
        config: Arc<Config>,
        registry: Arc<ExecutorRegistry>,
        llm_client: Arc<dyn LlmClient>,
        tool_registry: Arc<ToolRegistry>,
    ) -> Result<()> {
        // Get executor for this mode
        let executor = registry.get(plan.execution_mode).context(format!(
            "No executor registered for mode: {:?}",
            plan.execution_mode
        ))?;

        // Create executor context
        let ctx = ExecutorContext::new(
            project_path,
            config,
            event_tx.clone(),
            plan.id.clone(),
            plan.execution_mode,
            llm_client,
            tool_registry,
        );

        // Mark plan as executing
        plan.mark_executing();
        event_tx
            .send(PlanEvent::PlanStarted {
                plan_id: plan.id.clone(),
            })
            .ok();

        // Prepare
        executor
            .prepare(plan, &ctx)
            .await
            .context("Failed to prepare executor")?;

        // Execute groups in order
        for group_idx in 0..plan.groups.len() {
            // Check dependencies
            let deps_met = {
                let group = &plan.groups[group_idx];
                plan.dependencies_met(group)
            };

            if !deps_met {
                // Skip group if dependencies not met
                for step in &mut plan.groups[group_idx].steps {
                    step.status = StepStatus::Skipped;
                }
                continue;
            }

            let group = &plan.groups[group_idx];
            let group_id = group.id.clone();
            let parallel_count = group.step_count();

            // Emit group started
            event_tx
                .send(PlanEvent::GroupStarted {
                    plan_id: plan.id.clone(),
                    group_id: group_id.clone(),
                    parallel_count,
                })
                .ok();

            // Execute steps
            let results = if executor.supports_parallel() && parallel_count > 1 {
                // Parallel execution
                executor.execute_steps(&group.steps, &group_id, &ctx).await
            } else {
                // Sequential execution
                let mut results = Vec::new();
                for step in &group.steps {
                    results.push(executor.execute_step(step, &group_id, &ctx).await);
                }
                results
            };

            // Update step statuses
            let group = &mut plan.groups[group_idx];
            for (step, result) in group.steps.iter_mut().zip(results) {
                match result {
                    Ok(r) => {
                        step.status = if r.success {
                            StepStatus::Completed
                        } else {
                            StepStatus::Failed
                        };
                        step.result = Some(r);
                    }
                    Err(e) => {
                        step.status = StepStatus::Failed;
                        step.result = Some(super::types::StepResult {
                            success: false,
                            output: String::new(),
                            error: Some(e.to_string()),
                            duration_ms: 0,
                            files_modified: Vec::new(),
                        });
                    }
                }
            }

            // Emit group completed
            let group_success = group.is_successful();
            event_tx
                .send(PlanEvent::GroupCompleted {
                    plan_id: plan.id.clone(),
                    group_id,
                    success: group_success,
                })
                .ok();
        }

        // Finalize
        executor
            .finalize(plan, &ctx)
            .await
            .context("Failed to finalize executor")?;

        // Mark plan as completed
        plan.mark_completed();

        // Emit completion
        event_tx
            .send(PlanEvent::PlanCompleted {
                plan_id: plan.id.clone(),
                success: plan.status == PlanStatus::Completed,
                summary: plan.summary(),
            })
            .ok();

        Ok(())
    }
}

/// Builder for creating a plan runner with common configurations
pub struct PlanRunnerBuilder {
    project_path: PathBuf,
    config: Arc<Config>,
    registry: Arc<ExecutorRegistry>,
    llm_client: Option<Arc<dyn LlmClient>>,
    tool_registry: Option<Arc<ToolRegistry>>,
    requires_approval: bool,
}

impl PlanRunnerBuilder {
    /// Create a new builder
    pub fn new(project_path: PathBuf, config: Arc<Config>) -> Self {
        Self {
            project_path,
            config,
            registry: Arc::new(ExecutorRegistry::new()),
            llm_client: None,
            tool_registry: None,
            requires_approval: false,
        }
    }

    /// Set the executor registry
    pub fn with_registry(mut self, registry: Arc<ExecutorRegistry>) -> Self {
        self.registry = registry;
        self
    }

    /// Set the LLM client
    pub fn with_llm_client(mut self, llm_client: Arc<dyn LlmClient>) -> Self {
        self.llm_client = Some(llm_client);
        self
    }

    /// Set the tool registry
    pub fn with_tool_registry(mut self, tool_registry: Arc<ToolRegistry>) -> Self {
        self.tool_registry = Some(tool_registry);
        self
    }

    /// Add an executor for a mode
    pub fn with_executor(mut self, mode: ExecutionMode, executor: Arc<dyn PlanExecutor>) -> Self {
        Arc::get_mut(&mut self.registry)
            .expect("Registry already shared")
            .register(mode, executor);
        self
    }

    /// Require approval before execution
    pub fn require_approval(mut self) -> Self {
        self.requires_approval = true;
        self
    }

    /// Build the runner
    pub fn build(self) -> PlanRunner {
        let llm_client = self.llm_client.expect("LLM client must be set");
        let tool_registry = self.tool_registry.expect("Tool registry must be set");

        PlanRunner::new(
            self.project_path,
            self.config,
            self.registry,
            llm_client,
            tool_registry,
        )
        .with_approval(self.requires_approval)
    }
}

/// Create a default executor registry with all built-in executors
pub fn create_default_registry() -> ExecutorRegistry {
    let mut registry = ExecutorRegistry::new();

    // Note: Actual executor implementations will be added in Phase 5
    // For now, we just create an empty registry
    // registry.register(ExecutionMode::Direct, Arc::new(DirectExecutor::new()));
    // registry.register(ExecutionMode::Subagent, Arc::new(SubagentPlanExecutor::new()));
    // registry.register(ExecutionMode::Orchestration, Arc::new(OrchestrationExecutor::new()));

    registry
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::unified_planning::types::{StepGroup, UnifiedStep};

    fn create_test_plan() -> UnifiedPlan {
        let step1 = UnifiedStep::new("s1", "Step 1").with_instructions("Do step 1");
        let step2 = UnifiedStep::new("s2", "Step 2").with_instructions("Do step 2");

        UnifiedPlan::new("test-plan", "Test request")
            .with_title("Test Plan")
            .with_mode(ExecutionMode::Direct)
            .add_group(StepGroup::new("g1").add_step(step1))
            .add_group(StepGroup::new("g2").depends_on("g1").add_step(step2))
    }

    #[test]
    fn test_plan_runner_creation() {
        use crate::llm::create_client;

        let config = Arc::new(Config::default());
        let registry = Arc::new(ExecutorRegistry::new());
        let llm_client = Arc::new(create_client(&config).unwrap());
        let tool_registry = Arc::new(ToolRegistry::new());

        let runner = PlanRunner::new(
            PathBuf::from("/tmp"),
            config,
            registry,
            llm_client,
            tool_registry,
        );

        assert!(!runner.requires_approval);
    }

    #[test]
    fn test_plan_runner_with_approval() {
        use crate::llm::create_client;

        let config = Arc::new(Config::default());
        let registry = Arc::new(ExecutorRegistry::new());
        let llm_client = Arc::new(create_client(&config).unwrap());
        let tool_registry = Arc::new(ToolRegistry::new());

        let runner = PlanRunner::new(
            PathBuf::from("/tmp"),
            config,
            registry,
            llm_client,
            tool_registry,
        )
        .with_approval(true);

        assert!(runner.requires_approval);
    }

    #[test]
    fn test_plan_builder() {
        use crate::llm::create_client;

        let config = Arc::new(Config::default());
        let llm_client = Arc::new(create_client(&config).unwrap());
        let tool_registry = Arc::new(ToolRegistry::new());

        let runner = PlanRunnerBuilder::new(PathBuf::from("/tmp"), config)
            .with_llm_client(llm_client)
            .with_tool_registry(tool_registry)
            .require_approval()
            .build();

        assert!(runner.requires_approval);
    }

    #[test]
    fn test_test_plan_structure() {
        let plan = create_test_plan();
        assert_eq!(plan.groups.len(), 2);
        assert_eq!(plan.total_steps(), 2);
        assert_eq!(plan.groups[1].depends_on, vec!["g1"]);
    }
}
