//! CLI enhancements for self-orchestration
//!
//! This module adds command-line options to enable and configure
//! safe-coder's self-orchestration capabilities.

use clap::{Args, Subcommand};
use std::path::PathBuf;

/// Self-orchestration related CLI commands
#[derive(Subcommand, Debug, Clone)]
pub enum SelfOrchestrationCommand {
    /// Configure self-orchestration settings
    Configure {
        /// Maximum number of concurrent safe-coder instances
        #[arg(long, default_value = "4")]
        max_instances: usize,
        
        /// Enable automatic role detection for specialized instances
        #[arg(long, default_value = "true")]
        auto_roles: bool,
        
        /// Enable hierarchical orchestration (instances can spawn sub-instances)
        #[arg(long, default_value = "true")]
        hierarchical: bool,
        
        /// Worker strategy to use
        #[arg(long, default_value = "task-based", value_enum)]
        strategy: WorkerStrategyArg,
        
        /// Delay between starting instances (milliseconds)
        #[arg(long, default_value = "300")]
        start_delay_ms: u64,
        
        /// Save configuration to file
        #[arg(long)]
        save_config: bool,
    },
    
    /// Show current self-orchestration status
    Status,
    
    /// Execute a task using self-orchestration
    Execute {
        /// The task description
        task: String,
        
        /// Force a specific number of instances
        #[arg(long)]
        instances: Option<usize>,
        
        /// Force specific roles for instances
        #[arg(long, value_delimiter = ',')]
        roles: Vec<InstanceRoleArg>,
        
        /// Execution mode
        #[arg(long, default_value = "plan", value_enum)]
        mode: ExecutionModeArg,
        
        /// Show detailed progress
        #[arg(long)]
        verbose: bool,
    },
    
    /// Stop all running self-orchestrated instances
    Stop {
        /// Force stop without graceful shutdown
        #[arg(long)]
        force: bool,
    },
    
    /// Run a self-orchestration demo
    Demo {
        /// Type of demo to run
        #[arg(value_enum)]
        demo_type: DemoType,
    },
}

/// Worker strategy arguments
#[derive(clap::ValueEnum, Debug, Clone)]
pub enum WorkerStrategyArg {
    Single,
    RoundRobin,
    TaskBased,
    LoadBalanced,
}

/// Instance role arguments
#[derive(clap::ValueEnum, Debug, Clone)]
pub enum InstanceRoleArg {
    General,
    Test,
    Documentation,
    Refactoring,
    CodeGenerator,
    BugFixer,
    Performance,
    MetaOrchestrator,
}

/// Execution mode arguments
#[derive(clap::ValueEnum, Debug, Clone)]
pub enum ExecutionModeArg {
    Plan, // Show plan and ask for approval
    Act,  // Execute immediately
}

/// Demo types
#[derive(clap::ValueEnum, Debug, Clone)]
pub enum DemoType {
    FeatureDevelopment,
    LargeRefactoring,
    BugHunt,
    Performance,
    Documentation,
}

/// Self-orchestration CLI arguments
#[derive(Args, Debug, Clone)]
pub struct SelfOrchestrationArgs {
    /// Enable self-orchestration mode
    #[arg(long)]
    pub self_orchestrate: bool,
    
    /// Maximum concurrent self-orchestrated instances
    #[arg(long, default_value = "4")]
    pub max_self_instances: usize,
    
    /// Enable automatic role detection
    #[arg(long, default_value = "true")]
    pub auto_detect_roles: bool,
    
    /// Specific roles to use (overrides auto-detection)
    #[arg(long, value_delimiter = ',')]
    pub force_roles: Vec<InstanceRoleArg>,
    
    /// Self-orchestration configuration file
    #[arg(long)]
    pub self_orchestration_config: Option<PathBuf>,
    
    /// Show self-orchestration plan without executing
    #[arg(long)]
    pub show_self_orchestration_plan: bool,
}

impl From<WorkerStrategyArg> for crate::orchestrator::WorkerStrategy {
    fn from(arg: WorkerStrategyArg) -> Self {
        match arg {
            WorkerStrategyArg::Single => Self::SingleWorker,
            WorkerStrategyArg::RoundRobin => Self::RoundRobin,
            WorkerStrategyArg::TaskBased => Self::TaskBased,
            WorkerStrategyArg::LoadBalanced => Self::LoadBalanced,
        }
    }
}

impl From<InstanceRoleArg> for crate::orchestrator::self_orchestration::InstanceRole {
    fn from(arg: InstanceRoleArg) -> Self {
        match arg {
            InstanceRoleArg::General => Self::GeneralWorker,
            InstanceRoleArg::Test => Self::TestSpecialist,
            InstanceRoleArg::Documentation => Self::DocumentationSpecialist,
            InstanceRoleArg::Refactoring => Self::RefactoringSpecialist,
            InstanceRoleArg::CodeGenerator => Self::CodeGenerator,
            InstanceRoleArg::BugFixer => Self::BugFixer,
            InstanceRoleArg::Performance => Self::PerformanceOptimizer,
            InstanceRoleArg::MetaOrchestrator => Self::MetaOrchestrator,
        }
    }
}

impl From<ExecutionModeArg> for crate::approval::ExecutionMode {
    fn from(arg: ExecutionModeArg) -> Self {
        match arg {
            ExecutionModeArg::Plan => Self::Plan,
            ExecutionModeArg::Act => Self::Act,
        }
    }
}