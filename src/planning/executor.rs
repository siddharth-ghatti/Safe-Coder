//! Plan executor
//!
//! Executes plan steps inline. Subagent support is disabled for now
//! while we perfect the planning and execution flow.

use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;
use std::time::Instant;
use tokio::sync::mpsc;

use crate::config::Config;

use super::types::{PlanEvent, PlanStatus, PlanStepStatus, StepAssignment, TaskPlan};

/// Executes a task plan step-by-step
pub struct PlanExecutor {
    /// Project path for tool execution
    _project_path: PathBuf,
    /// Configuration
    _config: Config,
    /// Event sender for progress updates
    event_tx: mpsc::UnboundedSender<PlanEvent>,
}

impl PlanExecutor {
    /// Create a new plan executor
    pub fn new(
        project_path: PathBuf,
        config: Config,
        event_tx: mpsc::UnboundedSender<PlanEvent>,
    ) -> Self {
        Self {
            _project_path: project_path,
            _config: config,
            event_tx,
        }
    }

    /// Execute a plan - all steps are executed inline
    pub async fn execute(&self, plan: &mut TaskPlan) -> Result<()> {
        plan.status = PlanStatus::Executing;
        plan.started_at = Some(Utc::now());

        // Execute steps in order, respecting dependencies
        let total_steps = plan.steps.len();
        for i in 0..total_steps {
            // Check if dependencies are met
            if !plan.dependencies_met(&plan.steps[i]) {
                // Skip for now, will be handled in a more sophisticated scheduler later
                continue;
            }

            // Get step info for events
            let step_id = plan.steps[i].id.clone();
            let step_description = plan.steps[i].active_description.clone();
            let step_instructions = plan.steps[i].instructions.clone();

            // Mark step as in progress
            plan.steps[i].status = PlanStepStatus::InProgress;

            // Emit step started event
            let _ = self.event_tx.send(PlanEvent::StepStarted {
                plan_id: plan.id.clone(),
                step_id: step_id.clone(),
                description: step_description.clone(),
            });

            let start_time = Instant::now();

            // Execute inline - subagents are disabled
            // The session's main loop will handle the actual tool calls
            let result = self.execute_inline(&step_instructions).await;

            let duration = start_time.elapsed().as_millis() as u64;

            // Update step status based on result
            match result {
                Ok(output) => {
                    plan.steps[i].status = PlanStepStatus::Completed;
                    plan.steps[i].output = Some(output.clone());
                    plan.steps[i].duration_ms = Some(duration);

                    let _ = self.event_tx.send(PlanEvent::StepCompleted {
                        plan_id: plan.id.clone(),
                        step_id: step_id.clone(),
                        success: true,
                        output: Some(output),
                        error: None,
                    });
                }
                Err(e) => {
                    plan.steps[i].status = PlanStepStatus::Failed;
                    plan.steps[i].error = Some(e.to_string());
                    plan.steps[i].duration_ms = Some(duration);

                    let _ = self.event_tx.send(PlanEvent::StepCompleted {
                        plan_id: plan.id.clone(),
                        step_id: step_id.clone(),
                        success: false,
                        output: None,
                        error: Some(e.to_string()),
                    });

                    // Continue with other steps even if one fails
                }
            }
        }

        // Determine final status
        let all_completed = plan
            .steps
            .iter()
            .all(|s| s.status == PlanStepStatus::Completed);
        let any_failed = plan
            .steps
            .iter()
            .any(|s| s.status == PlanStepStatus::Failed);

        plan.status = if all_completed {
            PlanStatus::Completed
        } else if any_failed {
            PlanStatus::Failed
        } else {
            PlanStatus::Failed // Some steps not completed
        };

        plan.completed_at = Some(Utc::now());

        // Emit plan completed event
        let _ = self.event_tx.send(PlanEvent::PlanCompleted {
            plan_id: plan.id.clone(),
            success: plan.status == PlanStatus::Completed,
            summary: plan.summary(),
        });

        Ok(())
    }

    /// Execute a step inline (returns instructions for session to execute)
    async fn execute_inline(&self, instructions: &str) -> Result<String> {
        // For inline execution, we return the instructions
        // The session's main loop will handle the actual tool calls
        Ok(instructions.to_string())
    }
}

/// Wrapper for executing plans with approval flow
pub struct PlanRunner {
    executor: PlanExecutor,
    requires_approval: bool,
}

impl PlanRunner {
    /// Create a runner that requires approval (PLAN mode)
    pub fn with_approval(
        project_path: PathBuf,
        config: Config,
        event_tx: mpsc::UnboundedSender<PlanEvent>,
    ) -> Self {
        Self {
            executor: PlanExecutor::new(project_path, config, event_tx),
            requires_approval: true,
        }
    }

    /// Create a runner that executes immediately (BUILD mode)
    pub fn immediate(
        project_path: PathBuf,
        config: Config,
        event_tx: mpsc::UnboundedSender<PlanEvent>,
    ) -> Self {
        Self {
            executor: PlanExecutor::new(project_path, config, event_tx),
            requires_approval: false,
        }
    }

    /// Run a plan (with or without approval based on mode)
    pub async fn run(&self, plan: &mut TaskPlan, approved: bool) -> Result<()> {
        if self.requires_approval {
            if !approved {
                plan.status = PlanStatus::AwaitingApproval;
                let _ = self.executor.event_tx.send(PlanEvent::AwaitingApproval {
                    plan_id: plan.id.clone(),
                });
                return Ok(());
            }

            // User approved
            let _ = self.executor.event_tx.send(PlanEvent::PlanApproved {
                plan_id: plan.id.clone(),
            });
        }

        self.executor.execute(plan).await
    }

    /// Approve a pending plan and execute it
    pub async fn approve_and_execute(&self, plan: &mut TaskPlan) -> Result<()> {
        let _ = self.executor.event_tx.send(PlanEvent::PlanApproved {
            plan_id: plan.id.clone(),
        });
        self.executor.execute(plan).await
    }

    /// Reject a pending plan
    pub fn reject(&self, plan: &mut TaskPlan) {
        plan.status = PlanStatus::Cancelled;
        let _ = self.executor.event_tx.send(PlanEvent::PlanRejected {
            plan_id: plan.id.clone(),
        });
    }
}
