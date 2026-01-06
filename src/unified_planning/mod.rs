//! Unified Planning System
//!
//! This module provides a centralized planning system that works across all execution modes:
//! - Direct: Inline execution in the current session
//! - Subagent: Parallel execution using internal specialized agents
//! - Orchestration: Parallel execution using external CLI workers in git worktrees
//!
//! The planning system:
//! 1. Uses LLM to decompose user requests into step groups
//! 2. Is mode-aware - the LLM knows the execution capabilities
//! 3. Supports parallel execution where the mode allows
//! 4. Hands off to the appropriate executor based on the plan

pub mod executor_trait;
pub mod executors;
pub mod integration;
pub mod planner;
pub mod runner;
pub mod types;

pub use executor_trait::*;
pub use integration::{create_planner, create_runner, plan_and_execute, suggest_execution_mode};
pub use planner::UnifiedPlanner;
pub use runner::PlanRunner;
pub use types::*;
