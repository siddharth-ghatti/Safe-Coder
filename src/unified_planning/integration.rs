//! Integration helpers for the unified planning system
//!
//! This module provides helper functions to integrate the unified planning
//! system with existing components like Session and Orchestrator.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::llm::LlmClient;
use crate::tools::ToolRegistry;

use super::executors::create_full_registry;
use super::planner::UnifiedPlanner;
use super::runner::{PlanRunner, PlanRunnerBuilder};
use super::types::{ExecutionMode, PlanEvent, UnifiedPlan};

/// Create a unified planner for the given execution mode
pub fn create_planner(mode: ExecutionMode) -> UnifiedPlanner {
    UnifiedPlanner::new(mode)
}

/// Create a plan runner with all built-in executors registered
pub fn create_runner(
    project_path: PathBuf,
    config: Arc<Config>,
    llm_client: Arc<dyn LlmClient>,
    tool_registry: Arc<ToolRegistry>,
) -> PlanRunner {
    let registry = Arc::new(create_full_registry());

    PlanRunnerBuilder::new(project_path, config)
        .with_registry(registry)
        .with_llm_client(llm_client)
        .with_tool_registry(tool_registry)
        .build()
}

/// Create a plan runner that requires user approval
pub fn create_runner_with_approval(
    project_path: PathBuf,
    config: Arc<Config>,
    llm_client: Arc<dyn LlmClient>,
    tool_registry: Arc<ToolRegistry>,
) -> PlanRunner {
    let registry = Arc::new(create_full_registry());

    PlanRunnerBuilder::new(project_path, config)
        .with_registry(registry)
        .with_llm_client(llm_client)
        .with_tool_registry(tool_registry)
        .require_approval()
        .build()
}

/// High-level function to plan and execute a task
///
/// This is the main entry point for using the unified planning system.
/// It handles:
/// 1. Creating a plan using the LLM
/// 2. Optionally getting user approval
/// 3. Executing the plan with the appropriate executor
/// 4. Returning the completed plan and event stream
pub async fn plan_and_execute(
    request: &str,
    mode: ExecutionMode,
    llm_client: Arc<dyn LlmClient>,
    tool_registry: Arc<ToolRegistry>,
    project_path: PathBuf,
    config: Arc<Config>,
    project_context: Option<&str>,
    require_approval: bool,
) -> anyhow::Result<(UnifiedPlan, mpsc::UnboundedReceiver<PlanEvent>)> {
    // Create planner and runner
    let planner = create_planner(mode);
    let runner = if require_approval {
        create_runner_with_approval(project_path, config, llm_client.clone(), tool_registry)
    } else {
        create_runner(project_path, config, llm_client.clone(), tool_registry)
    };

    // Create the plan
    let plan = planner
        .create_plan(llm_client.as_ref(), request, project_context)
        .await?;

    // Execute the plan
    runner.execute(plan).await
}

/// Determine the best execution mode based on task characteristics
///
/// This analyzes the request and suggests an appropriate execution mode.
/// The heuristics are:
/// - Simple, single-file tasks -> Direct
/// - Medium complexity with focused subtasks -> Subagent
/// - Large, independent modules -> Orchestration
pub fn suggest_execution_mode(
    request: &str,
    estimated_files: usize,
    has_parallel_work: bool,
) -> ExecutionMode {
    // Keywords suggesting orchestration (complex, large scope)
    let orchestration_keywords = [
        "refactor",
        "redesign",
        "rewrite",
        "implement",
        "create",
        "add feature",
        "multiple",
        "all files",
        "entire",
        "comprehensive",
    ];

    // Keywords suggesting subagent (focused, specialized)
    let subagent_keywords = [
        "test", "analyze", "document", "review", "check", "fix", "update",
    ];

    let request_lower = request.to_lowercase();

    // Check for orchestration indicators
    let is_orchestration = orchestration_keywords
        .iter()
        .any(|k| request_lower.contains(k))
        || estimated_files > 5
        || (has_parallel_work && estimated_files > 2);

    // Check for subagent indicators
    let is_subagent = subagent_keywords.iter().any(|k| request_lower.contains(k))
        || has_parallel_work
        || estimated_files > 1;

    if is_orchestration {
        ExecutionMode::Orchestration
    } else if is_subagent {
        ExecutionMode::Subagent
    } else {
        ExecutionMode::Direct
    }
}

/// Convert unified PlanEvent to legacy planning PlanEvent
///
/// This bridges the unified planning system with the existing session event infrastructure
fn convert_to_legacy_event(event: &PlanEvent) -> Option<crate::planning::PlanEvent> {
    use crate::planning::{
        PlanEvent as LegacyEvent, PlanStatus, PlanStep, PlanStepStatus, TaskPlan,
    };

    match event {
        PlanEvent::PlanCreated { plan, .. } => {
            // Convert UnifiedPlan to legacy TaskPlan
            Some(LegacyEvent::PlanCreated {
                plan: plan.to_legacy_plan(),
            })
        }
        PlanEvent::PlanAwaitingApproval { plan_id } => Some(LegacyEvent::AwaitingApproval {
            plan_id: plan_id.clone(),
        }),
        PlanEvent::PlanApproved { plan_id } => Some(LegacyEvent::PlanApproved {
            plan_id: plan_id.clone(),
        }),
        PlanEvent::PlanRejected { plan_id, .. } => Some(LegacyEvent::PlanRejected {
            plan_id: plan_id.clone(),
        }),
        PlanEvent::StepStarted {
            plan_id,
            step_id,
            description,
            ..
        } => Some(LegacyEvent::StepStarted {
            plan_id: plan_id.clone(),
            step_id: step_id.clone(),
            description: description.clone(),
        }),
        PlanEvent::StepProgress {
            plan_id,
            step_id,
            message,
        } => Some(LegacyEvent::StepProgress {
            plan_id: plan_id.clone(),
            step_id: step_id.clone(),
            message: message.clone(),
        }),
        PlanEvent::StepCompleted {
            plan_id,
            step_id,
            success,
            ..
        } => Some(LegacyEvent::StepCompleted {
            plan_id: plan_id.clone(),
            step_id: step_id.clone(),
            success: *success,
            output: None,
            error: None,
        }),
        PlanEvent::PlanCompleted {
            plan_id,
            success,
            summary,
        } => Some(LegacyEvent::PlanCompleted {
            plan_id: plan_id.clone(),
            success: *success,
            summary: summary.clone(),
        }),
        // FileModified doesn't have a legacy equivalent - it's forwarded separately
        PlanEvent::FileModified { .. } => None,
        // Events that don't have legacy equivalents
        PlanEvent::PlanStarted { .. }
        | PlanEvent::GroupStarted { .. }
        | PlanEvent::GroupCompleted { .. } => None,
    }
}

/// Convert session event channel to plan event channel
///
/// Useful for integrating with existing session event infrastructure
pub fn create_plan_event_forwarder(
    session_tx: mpsc::UnboundedSender<crate::session::SessionEvent>,
) -> mpsc::UnboundedSender<PlanEvent> {
    let (plan_tx, mut plan_rx) = mpsc::unbounded_channel::<PlanEvent>();

    tokio::spawn(async move {
        while let Some(event) = plan_rx.recv().await {
            // Convert and forward plan events to session
            if let Some(legacy_event) = convert_to_legacy_event(&event) {
                let _ = session_tx.send(crate::session::SessionEvent::Plan(legacy_event));
            }
        }
    });

    plan_tx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_suggest_mode_simple() {
        // No keywords, single file, no parallelism -> Direct
        let mode = suggest_execution_mode("change the color", 1, false);
        assert_eq!(mode, ExecutionMode::Direct);
    }

    #[test]
    fn test_suggest_mode_subagent_keyword() {
        // "fix" is a subagent keyword, single file -> Subagent
        let mode = suggest_execution_mode("fix typo in readme", 1, false);
        assert_eq!(mode, ExecutionMode::Subagent);
    }

    #[test]
    fn test_suggest_mode_testing_with_parallel() {
        // has_parallel_work=true with >2 files triggers orchestration
        let mode = suggest_execution_mode("add tests for the auth module", 3, true);
        assert_eq!(mode, ExecutionMode::Orchestration);
    }

    #[test]
    fn test_suggest_mode_testing_single_file() {
        // "test" keyword, single file, no parallelism -> Subagent
        let mode = suggest_execution_mode("add tests for the auth module", 1, false);
        assert_eq!(mode, ExecutionMode::Subagent);
    }

    #[test]
    fn test_suggest_mode_orchestration() {
        let mode = suggest_execution_mode("refactor the entire authentication system", 10, true);
        assert_eq!(mode, ExecutionMode::Orchestration);
    }

    #[test]
    fn test_suggest_mode_parallel() {
        let mode = suggest_execution_mode("update multiple files", 5, true);
        assert_eq!(mode, ExecutionMode::Orchestration);
    }
}
