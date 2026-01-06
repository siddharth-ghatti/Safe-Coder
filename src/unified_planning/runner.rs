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
    /// Whether to require user approval before execution
    requires_approval: bool,
    /// Approval callback (returns true if approved)
    approval_callback: Option<Box<dyn Fn(&UnifiedPlan) -> bool + Send + Sync>>,
}

impl PlanRunner {
    /// Create a new plan runner
    pub fn new(
        project_path: PathBuf,
        config: Arc<Config>,
        registry: Arc<ExecutorRegistry>,
    ) -> Self {
        Self {
            project_path,
            config,
            registry,
            requires_approval: false,
            approval_callback: None,
        }
    }

    /// Set whether approval is required
    pub fn with_approval(mut self, requires: bool) -> Self {
        self.requires_approval = requires;
        self
    }

    /// Set an approval callback
    pub fn with_approval_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&UnifiedPlan) -> bool + Send + Sync + 'static,
    {
        self.approval_callback = Some(Box::new(callback));
        self.requires_approval = true;
        self
    }

    /// Execute a plan and return the event receiver
    ///
    /// Events are emitted throughout execution for UI updates.
    pub async fn execute(
        &self,
        mut plan: UnifiedPlan,
    ) -> Result<(UnifiedPlan, UnboundedReceiver<PlanEvent>)> {
        // Create event channel
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        // Emit plan created event
        event_tx
            .send(PlanEvent::PlanCreated {
                plan_id: plan.id.clone(),
                title: plan.title.clone(),
                total_steps: plan.total_steps(),
                execution_mode: plan.execution_mode,
            })
            .ok();

        // Handle approval if required
        if self.requires_approval {
            plan.status = PlanStatus::AwaitingApproval;
            event_tx
                .send(PlanEvent::PlanAwaitingApproval {
                    plan_id: plan.id.clone(),
                })
                .ok();

            // Check approval
            let approved = if let Some(ref callback) = self.approval_callback {
                callback(&plan)
            } else {
                // Default: approved
                true
            };

            if !approved {
                event_tx
                    .send(PlanEvent::PlanRejected {
                        plan_id: plan.id.clone(),
                        reason: "User rejected the plan".to_string(),
                    })
                    .ok();
                plan.status = PlanStatus::Cancelled;
                return Ok((plan, event_rx));
            }

            event_tx
                .send(PlanEvent::PlanApproved {
                    plan_id: plan.id.clone(),
                })
                .ok();
        }

        // Execute the plan
        self.execute_plan(&mut plan, event_tx.clone()).await?;

        Ok((plan, event_rx))
    }

    /// Execute the plan using the appropriate executor
    async fn execute_plan(
        &self,
        plan: &mut UnifiedPlan,
        event_tx: UnboundedSender<PlanEvent>,
    ) -> Result<()> {
        // Get executor for this mode
        let executor = self.registry.get(plan.execution_mode).context(format!(
            "No executor registered for mode: {:?}",
            plan.execution_mode
        ))?;

        // Create executor context
        let ctx = ExecutorContext::new(
            self.project_path.clone(),
            self.config.clone(),
            event_tx.clone(),
            plan.id.clone(),
            plan.execution_mode,
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
    requires_approval: bool,
}

impl PlanRunnerBuilder {
    /// Create a new builder
    pub fn new(project_path: PathBuf, config: Arc<Config>) -> Self {
        Self {
            project_path,
            config,
            registry: Arc::new(ExecutorRegistry::new()),
            requires_approval: false,
        }
    }

    /// Set the executor registry
    pub fn with_registry(mut self, registry: Arc<ExecutorRegistry>) -> Self {
        self.registry = registry;
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
        PlanRunner::new(self.project_path, self.config, self.registry)
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
        let config = Arc::new(Config::default());
        let registry = Arc::new(ExecutorRegistry::new());
        let runner = PlanRunner::new(PathBuf::from("/tmp"), config, registry);

        assert!(!runner.requires_approval);
    }

    #[test]
    fn test_plan_runner_with_approval() {
        let config = Arc::new(Config::default());
        let registry = Arc::new(ExecutorRegistry::new());
        let runner = PlanRunner::new(PathBuf::from("/tmp"), config, registry).with_approval(true);

        assert!(runner.requires_approval);
    }

    #[test]
    fn test_plan_builder() {
        let config = Arc::new(Config::default());
        let runner = PlanRunnerBuilder::new(PathBuf::from("/tmp"), config)
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
