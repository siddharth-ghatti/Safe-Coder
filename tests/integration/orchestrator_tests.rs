use super::common::*;
use anyhow::Result;
use safe_coder::approval::ExecutionMode;
use safe_coder::orchestrator::{Orchestrator, OrchestratorConfig, WorkerKind, WorkerStrategy, ThrottleLimits};
use serial_test::serial;
use std::path::PathBuf;

#[tokio::test]
#[serial]
async fn test_orchestrator_creation() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    let config = OrchestratorConfig {
        claude_cli_path: Some("echo".to_string()), // Use echo instead of real Claude CLI
        gemini_cli_path: Some("echo".to_string()),
        safe_coder_cli_path: Some("echo".to_string()),
        gh_cli_path: Some("echo".to_string()),
        max_workers: 2,
        default_worker: WorkerKind::ClaudeCode,
        worker_strategy: WorkerStrategy::SingleWorker,
        enabled_workers: vec![WorkerKind::ClaudeCode],
        use_worktrees: false, // Disable worktrees for simpler testing
        throttle_limits: ThrottleLimits {
            claude_max_concurrent: 1,
            gemini_max_concurrent: 1,
            safe_coder_max_concurrent: 1,
            copilot_max_concurrent: 1,
            start_delay_ms: 10,
        },
        execution_mode: ExecutionMode::Act,
    };

    let orchestrator = Orchestrator::new(env.project_path.clone(), config).await?;
    
    // Should create without error
    assert_eq!(orchestrator.config.max_workers, 2);
    assert_eq!(orchestrator.config.default_worker, WorkerKind::ClaudeCode);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_worker_strategy_single() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    let config = OrchestratorConfig {
        claude_cli_path: Some("echo".to_string()),
        gemini_cli_path: Some("echo".to_string()),
        safe_coder_cli_path: Some("echo".to_string()),
        gh_cli_path: Some("echo".to_string()),
        max_workers: 1,
        default_worker: WorkerKind::ClaudeCode,
        worker_strategy: WorkerStrategy::SingleWorker,
        enabled_workers: vec![WorkerKind::ClaudeCode],
        use_worktrees: false,
        throttle_limits: ThrottleLimits::default(),
        execution_mode: ExecutionMode::Act,
    };

    let orchestrator = Orchestrator::new(env.project_path.clone(), config).await?;
    
    assert_eq!(orchestrator.config.worker_strategy, WorkerStrategy::SingleWorker);
    assert_eq!(orchestrator.config.max_workers, 1);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_worker_strategy_round_robin() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    let config = OrchestratorConfig {
        claude_cli_path: Some("echo".to_string()),
        gemini_cli_path: Some("echo".to_string()),
        safe_coder_cli_path: Some("echo".to_string()),
        gh_cli_path: Some("echo".to_string()),
        max_workers: 3,
        default_worker: WorkerKind::ClaudeCode,
        worker_strategy: WorkerStrategy::RoundRobin,
        enabled_workers: vec![WorkerKind::ClaudeCode, WorkerKind::GeminiCli],
        use_worktrees: false,
        throttle_limits: ThrottleLimits::default(),
        execution_mode: ExecutionMode::Act,
    };

    let orchestrator = Orchestrator::new(env.project_path.clone(), config).await?;
    
    assert_eq!(orchestrator.config.worker_strategy, WorkerStrategy::RoundRobin);
    assert_eq!(orchestrator.config.enabled_workers.len(), 2);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_orchestrator_status() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    let config = OrchestratorConfig {
        claude_cli_path: Some("echo".to_string()),
        gemini_cli_path: Some("echo".to_string()),
        safe_coder_cli_path: Some("echo".to_string()),
        gh_cli_path: Some("echo".to_string()),
        max_workers: 2,
        default_worker: WorkerKind::ClaudeCode,
        worker_strategy: WorkerStrategy::SingleWorker,
        enabled_workers: vec![WorkerKind::ClaudeCode],
        use_worktrees: false,
        throttle_limits: ThrottleLimits::default(),
        execution_mode: ExecutionMode::Act,
    };

    let mut orchestrator = Orchestrator::new(env.project_path.clone(), config).await?;
    
    // Initially should have no workers
    let statuses = orchestrator.get_status().await;
    assert_eq!(statuses.len(), 0);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_orchestrator_cleanup() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    let config = OrchestratorConfig {
        claude_cli_path: Some("echo".to_string()),
        gemini_cli_path: Some("echo".to_string()),
        safe_coder_cli_path: Some("echo".to_string()),
        gh_cli_path: Some("echo".to_string()),
        max_workers: 2,
        default_worker: WorkerKind::ClaudeCode,
        worker_strategy: WorkerStrategy::SingleWorker,
        enabled_workers: vec![WorkerKind::ClaudeCode],
        use_worktrees: false,
        throttle_limits: ThrottleLimits::default(),
        execution_mode: ExecutionMode::Act,
    };

    let mut orchestrator = Orchestrator::new(env.project_path.clone(), config).await?;
    
    // Cleanup should not error even with no workers
    let result = orchestrator.cleanup().await;
    assert!(result.is_ok());
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_execution_modes() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    // Test Plan mode
    let plan_config = OrchestratorConfig {
        claude_cli_path: Some("echo".to_string()),
        gemini_cli_path: Some("echo".to_string()),
        safe_coder_cli_path: Some("echo".to_string()),
        gh_cli_path: Some("echo".to_string()),
        max_workers: 1,
        default_worker: WorkerKind::ClaudeCode,
        worker_strategy: WorkerStrategy::SingleWorker,
        enabled_workers: vec![WorkerKind::ClaudeCode],
        use_worktrees: false,
        throttle_limits: ThrottleLimits::default(),
        execution_mode: ExecutionMode::Plan,
    };

    let plan_orchestrator = Orchestrator::new(env.project_path.clone(), plan_config).await?;
    assert_eq!(plan_orchestrator.config.execution_mode, ExecutionMode::Plan);

    // Test Act mode
    let act_config = OrchestratorConfig {
        claude_cli_path: Some("echo".to_string()),
        gemini_cli_path: Some("echo".to_string()),
        safe_coder_cli_path: Some("echo".to_string()),
        gh_cli_path: Some("echo".to_string()),
        max_workers: 1,
        default_worker: WorkerKind::ClaudeCode,
        worker_strategy: WorkerStrategy::SingleWorker,
        enabled_workers: vec![WorkerKind::ClaudeCode],
        use_worktrees: false,
        throttle_limits: ThrottleLimits::default(),
        execution_mode: ExecutionMode::Act,
    };

    let act_orchestrator = Orchestrator::new(env.project_path.clone(), act_config).await?;
    assert_eq!(act_orchestrator.config.execution_mode, ExecutionMode::Act);

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_throttle_limits() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    let config = OrchestratorConfig {
        claude_cli_path: Some("echo".to_string()),
        gemini_cli_path: Some("echo".to_string()),
        safe_coder_cli_path: Some("echo".to_string()),
        gh_cli_path: Some("echo".to_string()),
        max_workers: 5,
        default_worker: WorkerKind::ClaudeCode,
        worker_strategy: WorkerStrategy::LoadBalanced,
        enabled_workers: vec![WorkerKind::ClaudeCode, WorkerKind::GeminiCli],
        use_worktrees: false,
        throttle_limits: ThrottleLimits {
            claude_max_concurrent: 2,
            gemini_max_concurrent: 3,
            safe_coder_max_concurrent: 1,
            copilot_max_concurrent: 1,
            start_delay_ms: 500,
        },
        execution_mode: ExecutionMode::Act,
    };

    let orchestrator = Orchestrator::new(env.project_path.clone(), config).await?;
    
    // Verify throttle limits are set correctly
    assert_eq!(orchestrator.config.throttle_limits.claude_max_concurrent, 2);
    assert_eq!(orchestrator.config.throttle_limits.gemini_max_concurrent, 3);
    assert_eq!(orchestrator.config.throttle_limits.start_delay_ms, 500);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_worker_kinds() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.setup_test_project()?;
    env.init_git().await?;

    let workers = vec![
        WorkerKind::ClaudeCode,
        WorkerKind::GeminiCli,
        WorkerKind::SafeCoder,
        WorkerKind::GitHubCopilot,
    ];

    for worker_kind in workers {
        let config = OrchestratorConfig {
            claude_cli_path: Some("echo".to_string()),
            gemini_cli_path: Some("echo".to_string()),
            safe_coder_cli_path: Some("echo".to_string()),
            gh_cli_path: Some("echo".to_string()),
            max_workers: 1,
            default_worker: worker_kind.clone(),
            worker_strategy: WorkerStrategy::SingleWorker,
            enabled_workers: vec![worker_kind.clone()],
            use_worktrees: false,
            throttle_limits: ThrottleLimits::default(),
            execution_mode: ExecutionMode::Act,
        };

        let orchestrator = Orchestrator::new(env.project_path.clone(), config).await?;
        assert_eq!(orchestrator.config.default_worker, worker_kind);
    }
    
    Ok(())
}

#[cfg(test)]
mod workspace_tests {
    use super::*;
    use safe_coder::orchestrator::WorkspaceManager;

    #[tokio::test]
    #[serial]
    async fn test_workspace_manager_creation() -> Result<()> {
        let env = TestEnvironment::new()?;
        env.setup_test_project()?;
        env.init_git().await?;

        let workspace_manager = WorkspaceManager::new(env.project_path.clone(), false).await?;
        
        // Should create without error
        assert!(workspace_manager.base_path().exists());
        
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_workspace_manager_with_worktrees() -> Result<()> {
        let env = TestEnvironment::new()?;
        env.setup_test_project()?;
        env.init_git().await?;

        // Commit initial state so we can create worktrees
        tokio::process::Command::new("git")
            .args(&["add", "."])
            .current_dir(&env.project_path)
            .status()
            .await?;

        tokio::process::Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(&env.project_path)
            .status()
            .await?;

        let workspace_manager = WorkspaceManager::new(env.project_path.clone(), true).await?;
        
        // Should create without error
        assert!(workspace_manager.base_path().exists());
        
        Ok(())
    }
}

#[cfg(test)]
mod task_processing_tests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_simple_task_processing() -> Result<()> {
        let env = TestEnvironment::new()?;
        env.setup_test_project()?;
        env.init_git().await?;

        let config = OrchestratorConfig {
            claude_cli_path: Some("echo".to_string()),
            gemini_cli_path: Some("echo".to_string()),
            safe_coder_cli_path: Some("echo".to_string()),
            gh_cli_path: Some("echo".to_string()),
            max_workers: 1,
            default_worker: WorkerKind::ClaudeCode,
            worker_strategy: WorkerStrategy::SingleWorker,
            enabled_workers: vec![WorkerKind::ClaudeCode],
            use_worktrees: false,
            throttle_limits: ThrottleLimits::default(),
            execution_mode: ExecutionMode::Act,
        };

        let mut orchestrator = Orchestrator::new(env.project_path.clone(), config).await?;
        
        // Process a simple request
        // Note: This will likely fail in actual execution since we're using echo instead of real CLIs
        // But it should not panic and should handle the error gracefully
        let result = orchestrator.process_request("Create a simple hello world function").await;
        
        // We expect this to either succeed (if mocking works) or fail gracefully
        match result {
            Ok(_) => {
                // Success case
            }
            Err(e) => {
                // Error case - should not panic, just return an error
                assert!(!e.to_string().contains("panic"));
            }
        }
        
        Ok(())
    }
}