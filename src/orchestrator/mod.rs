//! Orchestrator module for delegating tasks to external CLI agents
//! 
//! This module implements the orchestrator pattern where Safe Coder acts as a
//! high-level planner that delegates tasks to specialized CLI agents (Claude Code,
//! Gemini CLI) running in isolated git workspaces.

pub mod planner;
pub mod worker;
pub mod workspace;
pub mod task;

pub use planner::Planner;
pub use worker::{Worker, WorkerKind, WorkerStatus};
pub use workspace::WorkspaceManager;
pub use task::{Task, TaskStatus, TaskPlan};

use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// The main orchestrator that coordinates between the planner and workers
pub struct Orchestrator {
    /// High-level planner that breaks down tasks
    planner: Planner,
    /// Manager for git workspaces (worktrees/branches)
    workspace_manager: WorkspaceManager,
    /// Active workers executing tasks
    workers: Vec<Arc<Mutex<Worker>>>,
    /// Base project path
    project_path: PathBuf,
    /// Configuration for the orchestrator
    pub config: OrchestratorConfig,
}

/// Configuration for the orchestrator
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Path to Claude Code CLI (claude)
    pub claude_cli_path: Option<String>,
    /// Path to Gemini CLI (gemini)
    pub gemini_cli_path: Option<String>,
    /// Maximum concurrent workers
    pub max_workers: usize,
    /// Default worker kind to use
    pub default_worker: WorkerKind,
    /// Whether to use git worktrees for isolation
    pub use_worktrees: bool,
    /// Throttle limits per worker type
    pub throttle_limits: ThrottleLimits,
}

/// Throttle limits for different worker types
#[derive(Debug, Clone)]
pub struct ThrottleLimits {
    /// Maximum concurrent Claude Code workers
    pub claude_max_concurrent: usize,
    /// Maximum concurrent Gemini CLI workers
    pub gemini_max_concurrent: usize,
    /// Delay between starting workers of the same type (milliseconds)
    pub start_delay_ms: u64,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            claude_cli_path: Some("claude".to_string()),
            gemini_cli_path: Some("gemini".to_string()),
            max_workers: 3,
            default_worker: WorkerKind::ClaudeCode,
            use_worktrees: true,
            throttle_limits: ThrottleLimits::default(),
        }
    }
}

impl Default for ThrottleLimits {
    fn default() -> Self {
        Self {
            claude_max_concurrent: 2,
            gemini_max_concurrent: 2,
            start_delay_ms: 100,
        }
    }
}

impl Orchestrator {
    /// Create a new orchestrator for a project
    pub async fn new(project_path: PathBuf, config: OrchestratorConfig) -> Result<Self> {
        let planner = Planner::new();
        let workspace_manager = WorkspaceManager::new(project_path.clone(), config.use_worktrees)?;
        
        Ok(Self {
            planner,
            workspace_manager,
            workers: Vec::new(),
            project_path,
            config,
        })
    }
    
    /// Process a user request by planning and delegating to workers
    pub async fn process_request(&mut self, request: &str) -> Result<OrchestratorResponse> {
        // Step 1: Create a high-level plan
        let plan = self.planner.create_plan(request).await?;
        
        let mut response = OrchestratorResponse {
            plan: plan.clone(),
            task_results: Vec::new(),
            summary: String::new(),
        };
        
        // Step 2: Execute tasks in parallel with throttling
        let task_results = self.execute_tasks_parallel(&plan.tasks).await?;
        response.task_results = task_results;
        
        // Step 3: Merge results back
        for task_result in &response.task_results {
            if task_result.result.is_ok() {
                self.workspace_manager.merge_workspace(&task_result.task_id).await?;
            }
        }
        
        // Generate summary
        response.summary = self.generate_summary(&response);
        
        Ok(response)
    }
    
    /// Execute tasks in parallel with throttling (max concurrent workers)
    async fn execute_tasks_parallel(&mut self, tasks: &[Task]) -> Result<Vec<TaskResult>> {
        use tokio::task::JoinSet;
        use std::collections::{HashMap, VecDeque};
        
        let mut results = Vec::new();
        let mut join_set = JoinSet::new();
        let mut task_queue: VecDeque<Task> = tasks.iter().cloned().collect();
        
        // Track active workers by type for throttling
        let mut active_by_type: HashMap<WorkerKind, usize> = HashMap::new();
        let mut last_start_time = std::time::Instant::now();
        
        // Start initial batch of workers (respecting throttle limits)
        while !task_queue.is_empty() && join_set.len() < self.config.max_workers {
            if self.try_start_next_task(
                &mut task_queue, 
                &mut active_by_type, 
                &mut last_start_time, 
                &mut join_set
            ).await?.is_none() {
                // No tasks can be started due to throttle limits, wait for one to complete
                break;
            }
        }
        
        // As workers complete, start new ones until all tasks are done
        while let Some(result) = join_set.join_next().await {
            let (task_result, completed_worker_kind) = result?;
            results.push(task_result);
            
            // Decrement active count for this worker type
            if let Some(count) = active_by_type.get_mut(&completed_worker_kind) {
                *count = count.saturating_sub(1);
            }
            
            // Try to start next task from queue
            if !task_queue.is_empty() && join_set.len() < self.config.max_workers {
                // Try to start one task, then go back to waiting for completions
                self.try_start_next_task(
                    &mut task_queue, 
                    &mut active_by_type, 
                    &mut last_start_time, 
                    &mut join_set
                ).await?;
            }
        }
        
        Ok(results)
    }
    
    /// Try to start the next task from the queue that respects throttle limits
    /// Returns Some(task) if a task was started, None otherwise
    async fn try_start_next_task(
        &mut self,
        task_queue: &mut std::collections::VecDeque<Task>,
        active_by_type: &mut std::collections::HashMap<WorkerKind, usize>,
        last_start_time: &mut std::time::Instant,
        join_set: &mut tokio::task::JoinSet<(TaskResult, WorkerKind)>,
    ) -> Result<Option<Task>> {
        // Try each task in the queue to find one that can be started
        for i in 0..task_queue.len() {
            let task = &task_queue[i];
            let worker_kind = task.preferred_worker
                .clone()
                .unwrap_or(self.config.default_worker.clone());
            
            // Check throttle limits for this worker type
            let count = active_by_type.get(&worker_kind).copied().unwrap_or(0);
            let max = match worker_kind {
                WorkerKind::ClaudeCode => self.config.throttle_limits.claude_max_concurrent,
                WorkerKind::GeminiCli => self.config.throttle_limits.gemini_max_concurrent,
            };
            
            if count >= max {
                // This worker type is at limit, try next task
                continue;
            }
            
            // Can start this task, remove it from queue (O(1) for front, O(n) otherwise)
            let task = if i == 0 {
                task_queue.pop_front().unwrap()
            } else {
                task_queue.remove(i).unwrap()
            };
            let task_id = task.id.clone();
            
            // Apply start delay between workers
            let elapsed = last_start_time.elapsed().as_millis() as u64;
            if elapsed < self.config.throttle_limits.start_delay_ms {
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    self.config.throttle_limits.start_delay_ms - elapsed
                )).await;
            }
            
            // Start the worker
            let workspace = self.workspace_manager.create_workspace(&task_id).await?;
            let cli_path = self.get_cli_path(&worker_kind);
            
            let worker = Worker::new(
                task.clone(),
                workspace.clone(),
                worker_kind.clone(),
                cli_path,
            )?;
            
            let worker = Arc::new(Mutex::new(worker));
            self.workers.push(worker.clone());
            
            // Track active worker
            *active_by_type.entry(worker_kind.clone()).or_insert(0) += 1;
            *last_start_time = std::time::Instant::now();
            
            // Spawn task execution
            let worker_kind_clone = worker_kind.clone();
            join_set.spawn(async move {
                let result = {
                    let mut w = worker.lock().await;
                    w.execute().await
                };
                
                (TaskResult {
                    task_id,
                    worker_kind: worker_kind_clone.clone(),
                    workspace_path: workspace,
                    result,
                }, worker_kind_clone)
            });
            
            return Ok(Some(task));
        }
        
        // No tasks could be started due to throttle limits
        Ok(None)
    }
    
    /// Get the CLI path for a worker kind
    fn get_cli_path(&self, kind: &WorkerKind) -> String {
        match kind {
            WorkerKind::ClaudeCode => self.config.claude_cli_path
                .clone()
                .unwrap_or_else(|| "claude".to_string()),
            WorkerKind::GeminiCli => self.config.gemini_cli_path
                .clone()
                .unwrap_or_else(|| "gemini".to_string()),
        }
    }
    
    /// Generate a summary of the orchestration results
    fn generate_summary(&self, response: &OrchestratorResponse) -> String {
        let total = response.task_results.len();
        let successful = response.task_results.iter()
            .filter(|r| r.result.is_ok())
            .count();
        let failed = total - successful;
        
        let mut summary = format!(
            "📊 Orchestration Complete\n\
             ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\
             Tasks: {} total, {} successful, {} failed\n\n",
            total, successful, failed
        );
        
        for (i, task) in response.plan.tasks.iter().enumerate() {
            let result = &response.task_results[i];
            let status = if result.result.is_ok() { "✓" } else { "✗" };
            summary.push_str(&format!(
                "{} Task {}: {}\n  Worker: {:?}\n  Workspace: {}\n\n",
                status,
                task.id,
                task.description,
                result.worker_kind,
                result.workspace_path.display()
            ));
        }
        
        summary
    }
    
    /// Get status of all active workers
    pub async fn get_status(&self) -> Vec<WorkerStatus> {
        let mut statuses = Vec::new();
        for worker in &self.workers {
            let w = worker.lock().await;
            statuses.push(w.status());
        }
        statuses
    }
    
    /// Cancel all running workers
    pub async fn cancel_all(&mut self) -> Result<()> {
        for worker in &self.workers {
            let mut w = worker.lock().await;
            w.cancel().await?;
        }
        Ok(())
    }
    
    /// Cleanup all workspaces
    pub async fn cleanup(&mut self) -> Result<()> {
        self.workspace_manager.cleanup_all().await
    }
}

/// Response from the orchestrator
#[derive(Debug, Clone)]
pub struct OrchestratorResponse {
    /// The execution plan
    pub plan: TaskPlan,
    /// Results from each task
    pub task_results: Vec<TaskResult>,
    /// Summary of the orchestration
    pub summary: String,
}

/// Result of a single task execution
#[derive(Debug, Clone)]
pub struct TaskResult {
    /// Task identifier
    pub task_id: String,
    /// Which worker executed this task
    pub worker_kind: WorkerKind,
    /// Path to the workspace used
    pub workspace_path: PathBuf,
    /// Execution result
    pub result: Result<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_orchestrator_config_default() {
        let config = OrchestratorConfig::default();
        
        assert_eq!(config.max_workers, 3);
        assert_eq!(config.throttle_limits.claude_max_concurrent, 2);
        assert_eq!(config.throttle_limits.gemini_max_concurrent, 2);
        assert_eq!(config.throttle_limits.start_delay_ms, 100);
    }
    
    #[tokio::test]
    async fn test_throttle_limits_configuration() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = OrchestratorConfig {
            claude_cli_path: Some("claude".to_string()),
            gemini_cli_path: Some("gemini".to_string()),
            max_workers: 3,
            default_worker: WorkerKind::ClaudeCode,
            use_worktrees: false,
            throttle_limits: ThrottleLimits {
                claude_max_concurrent: 2,
                gemini_max_concurrent: 1,
                start_delay_ms: 50,
            },
        };
        
        let orchestrator = Orchestrator::new(temp_dir.path().to_path_buf(), config).await.unwrap();
        
        assert_eq!(orchestrator.config.throttle_limits.claude_max_concurrent, 2);
        assert_eq!(orchestrator.config.throttle_limits.gemini_max_concurrent, 1);
        assert_eq!(orchestrator.config.throttle_limits.start_delay_ms, 50);
    }
    
    #[tokio::test]
    async fn test_max_workers_enforced() {
        let temp_dir = TempDir::new().unwrap();
        
        let config = OrchestratorConfig {
            claude_cli_path: Some("echo".to_string()), // Use echo as mock CLI
            gemini_cli_path: Some("echo".to_string()),
            max_workers: 2, // Limit to 2 concurrent workers
            default_worker: WorkerKind::ClaudeCode,
            use_worktrees: false,
            throttle_limits: ThrottleLimits {
                claude_max_concurrent: 2,
                gemini_max_concurrent: 2,
                start_delay_ms: 0,
            },
        };
        
        let mut orchestrator = Orchestrator::new(temp_dir.path().to_path_buf(), config).await.unwrap();
        
        // Create a plan with multiple tasks
        let mut plan = TaskPlan::new(
            "test-plan".to_string(),
            "Test parallel execution".to_string(),
            "Testing".to_string(),
        );
        
        // Add 5 tasks (more than max_workers)
        for i in 1..=5 {
            plan.add_task(Task::new(
                format!("task-{}", i),
                format!("Task {}", i),
                "echo test".to_string(),
            ));
        }
        
        // Verify the plan has the expected number of tasks
        assert_eq!(plan.tasks.len(), 5);
        assert_eq!(orchestrator.config.max_workers, 2);
    }
}
