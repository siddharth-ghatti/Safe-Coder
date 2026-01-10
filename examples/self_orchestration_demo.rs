//! Practical examples of using safe-coder's self-orchestration capabilities
//!
//! This demonstrates how safe-coder can orchestrate multiple instances of itself
//! to handle complex development tasks in parallel.

use anyhow::Result;
use std::path::PathBuf;

use safe_coder::orchestrator::{
    Orchestrator, OrchestratorConfig, WorkerKind, WorkerStrategy, ThrottleLimits,
    self_orchestration::{SelfOrchestrationManager, InstanceRole, SelfOrchestrationStrategies}
};
use safe_coder::approval::ExecutionMode;

/// Example: Orchestrating a complete feature development
pub async fn orchestrate_feature_development(
    project_path: PathBuf,
    feature_description: &str,
) -> Result<()> {
    // Configure for pure self-orchestration
    let config = OrchestratorConfig {
        claude_cli_path: None,
        gemini_cli_path: None,
        safe_coder_cli_path: Some("safe-coder".to_string()),
        gh_cli_path: None,
        max_workers: 6, // Up to 6 parallel safe-coder instances
        default_worker: WorkerKind::SafeCoder,
        worker_strategy: WorkerStrategy::TaskBased, // Smart role assignment
        enabled_workers: vec![WorkerKind::SafeCoder],
        use_worktrees: true, // Critical for isolation
        throttle_limits: ThrottleLimits {
            claude_max_concurrent: 0,
            gemini_max_concurrent: 0,
            safe_coder_max_concurrent: 6, // All workers are safe-coder
            copilot_max_concurrent: 0,
            start_delay_ms: 300, // Stagger starts to avoid conflicts
        },
        execution_mode: ExecutionMode::Plan, // Show plan before executing
    };

    // Create orchestrator with self-orchestration capabilities
    let mut orchestrator = Orchestrator::new(project_path.clone(), config).await?;
    
    // Create self-orchestration manager with advanced strategies
    let strategies = SelfOrchestrationStrategies {
        auto_role_detection: true,  // Automatically assign specialist roles
        hierarchical_orchestration: true,  // Allow instances to spawn sub-instances
        dynamic_load_balancing: true,  // Balance load across instances
        inter_instance_communication: false,  // Future feature
    };

    let mut self_orchestrator = SelfOrchestrationManager::new(orchestrator.config.clone())
        .with_strategies(strategies);

    println!("🎯 Self-Orchestrating Feature Development");
    println!("==========================================");
    println!("Feature: {}", feature_description);
    println!();

    // Example complex feature request
    let complex_request = format!(r#"
Implement a new user authentication feature with the following requirements:

1. Create OAuth2 integration with Google and GitHub
2. Update the user model to store OAuth tokens and provider info  
3. Implement JWT token generation and validation
4. Create middleware for protecting API endpoints
5. Update all existing auth-related API endpoints
6. Write comprehensive unit and integration tests
7. Update API documentation
8. Create user-facing documentation
9. Add performance monitoring for auth operations
10. Implement rate limiting for auth endpoints

Context: {}

This is a large-scale feature that touches multiple parts of the system and can benefit from parallel development by specialized safe-coder instances.
"#, feature_description);

    // Process the request through the orchestrator
    let response = orchestrator.process_request(&complex_request).await?;

    println!("📊 Orchestration Results:");
    println!("{}", response.summary);

    // Monitor active instances
    let active_instances = self_orchestrator.get_active_instances();
    if !active_instances.is_empty() {
        println!("\n🔄 Active Self-Orchestrated Instances:");
        for (id, instance) in active_instances {
            println!("  • {} ({:?}): {}", id, instance.role, instance.task_id);
        }
    }

    Ok(())
}

/// Example: Self-orchestrating a large refactoring
pub async fn orchestrate_large_refactoring(
    project_path: PathBuf,
    refactoring_scope: &str,
) -> Result<()> {
    println!("🔧 Self-Orchestrating Large Refactoring");
    println!("========================================");
    
    let config = create_high_performance_config();
    let mut orchestrator = Orchestrator::new(project_path.clone(), config).await?;

    let refactoring_request = format!(r#"
Perform a large-scale refactoring of the codebase:

Scope: {}

Tasks:
1. Analyze current architecture and identify improvement opportunities
2. Create refactoring plan with dependency analysis
3. Refactor core modules in parallel (auth, api, database, utils)
4. Update all imports and references
5. Migrate tests to match new structure
6. Update documentation for architectural changes
7. Ensure all functionality is preserved
8. Run comprehensive test suite
9. Performance regression testing
10. Update deployment scripts if needed

This refactoring affects many files and should be done in parallel with careful coordination.
"#, refactoring_scope);

    let response = orchestrator.process_request(&refactoring_request).await?;
    
    println!("📊 Refactoring Results:");
    println!("{}", response.summary);

    Ok(())
}

/// Example: Self-orchestrating bug fixes across a codebase
pub async fn orchestrate_bug_hunt(
    project_path: PathBuf,
    bug_reports: Vec<String>,
) -> Result<()> {
    println!("🐛 Self-Orchestrating Bug Hunt");
    println!("==============================");

    let config = OrchestratorConfig {
        safe_coder_cli_path: Some("safe-coder".to_string()),
        max_workers: 4,
        default_worker: WorkerKind::SafeCoder,
        worker_strategy: WorkerStrategy::LoadBalanced,
        enabled_workers: vec![WorkerKind::SafeCoder],
        use_worktrees: true,
        throttle_limits: ThrottleLimits {
            safe_coder_max_concurrent: 4,
            start_delay_ms: 200,
            ..Default::default()
        },
        execution_mode: ExecutionMode::Act, // Auto-fix mode
        ..Default::default()
    };

    let mut orchestrator = Orchestrator::new(project_path, config).await?;

    for (i, bug_report) in bug_reports.iter().enumerate() {
        let bug_fix_request = format!(r#"
Bug Fix Request #{}: {}

Please:
1. Analyze the reported issue
2. Identify the root cause
3. Implement a targeted fix
4. Add regression tests
5. Verify the fix doesn't break existing functionality
6. Document the fix in the commit message

Focus on minimal, safe changes that directly address the issue.
"#, i + 1, bug_report);

        println!("🔍 Processing bug #{}: {}", i + 1, 
                 bug_report.chars().take(50).collect::<String>());

        let response = orchestrator.process_request(&bug_fix_request).await?;
        
        if response.task_results.iter().all(|r| r.result.is_ok()) {
            println!("  ✅ Bug #{} fixed successfully", i + 1);
        } else {
            println!("  ❌ Bug #{} fix failed", i + 1);
        }
    }

    Ok(())
}

/// Create a high-performance self-orchestration configuration
fn create_high_performance_config() -> OrchestratorConfig {
    OrchestratorConfig {
        claude_cli_path: None,
        gemini_cli_path: None,
        safe_coder_cli_path: Some("safe-coder".to_string()),
        gh_cli_path: None,
        max_workers: 8, // High concurrency
        default_worker: WorkerKind::SafeCoder,
        worker_strategy: WorkerStrategy::TaskBased,
        enabled_workers: vec![WorkerKind::SafeCoder],
        use_worktrees: true,
        throttle_limits: ThrottleLimits {
            claude_max_concurrent: 0,
            gemini_max_concurrent: 0,
            safe_coder_max_concurrent: 8, // Maximum self-orchestration
            copilot_max_concurrent: 0,
            start_delay_ms: 100, // Faster starts
        },
        execution_mode: ExecutionMode::Act,
    }
}

/// Example usage in main application
pub async fn demo_self_orchestration() -> Result<()> {
    let project_path = std::env::current_dir()?;

    println!("🚀 Safe-Coder Self-Orchestration Demo");
    println!("=====================================\n");

    // Demo 1: Feature development
    orchestrate_feature_development(
        project_path.clone(), 
        "Advanced authentication system with OAuth2, JWT, and rate limiting"
    ).await?;

    println!("\n" + "=".repeat(50) + "\n");

    // Demo 2: Large refactoring
    orchestrate_large_refactoring(
        project_path.clone(),
        "Migrate from monolithic to microservices architecture"
    ).await?;

    println!("\n" + "=".repeat(50) + "\n");

    // Demo 3: Bug hunt
    let bug_reports = vec![
        "Users can't log in after password reset".to_string(),
        "Memory leak in file processing module".to_string(),
        "Race condition in concurrent request handling".to_string(),
        "Incorrect validation in API endpoint".to_string(),
    ];

    orchestrate_bug_hunt(project_path, bug_reports).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_self_orchestration_config() {
        let config = create_high_performance_config();
        
        assert_eq!(config.max_workers, 8);
        assert_eq!(config.default_worker, WorkerKind::SafeCoder);
        assert_eq!(config.enabled_workers, vec![WorkerKind::SafeCoder]);
        assert_eq!(config.throttle_limits.safe_coder_max_concurrent, 8);
        assert!(config.use_worktrees);
    }

    #[tokio::test]
    async fn test_orchestrator_creation_with_self_orchestration() {
        let temp_dir = tempdir().unwrap();
        let config = create_high_performance_config();
        
        let orchestrator = Orchestrator::new(temp_dir.path().to_path_buf(), config).await;
        assert!(orchestrator.is_ok());
    }
}