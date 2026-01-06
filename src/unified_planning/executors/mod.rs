//! Executor Implementations
//!
//! This module contains the concrete implementations of the `PlanExecutor` trait
//! for each execution mode.

pub mod direct;
pub mod orchestration;
pub mod subagent;

pub use direct::DirectExecutor;
pub use orchestration::OrchestrationExecutor;
pub use subagent::SubagentPlanExecutor;

use std::sync::Arc;

use crate::unified_planning::{ExecutionMode, ExecutorRegistry, PlanExecutor};

/// Create a fully populated executor registry with all built-in executors
pub fn create_full_registry() -> ExecutorRegistry {
    let mut registry = ExecutorRegistry::new();

    registry.register(
        ExecutionMode::Direct,
        Arc::new(DirectExecutor::new()) as Arc<dyn PlanExecutor>,
    );
    registry.register(
        ExecutionMode::Subagent,
        Arc::new(SubagentPlanExecutor::new()) as Arc<dyn PlanExecutor>,
    );
    registry.register(
        ExecutionMode::Orchestration,
        Arc::new(OrchestrationExecutor::new()) as Arc<dyn PlanExecutor>,
    );

    registry
}
