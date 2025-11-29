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
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            claude_cli_path: Some("claude".to_string()),
            gemini_cli_path: Some("gemini".to_string()),
            max_workers: 3,
            default_worker: WorkerKind::ClaudeCode,
            use_worktrees: true,
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
        
        // Step 2: For each task in the plan, create a workspace and spawn a worker
        for task in &plan.tasks {
            // Create isolated workspace for this task
            let workspace = self.workspace_manager.create_workspace(&task.id).await?;
            
            // Determine which worker to use
            let worker_kind = task.preferred_worker
                .clone()
                .unwrap_or(self.config.default_worker.clone());
            
            // Create and start the worker
            let worker = Worker::new(
                task.clone(),
                workspace.clone(),
                worker_kind.clone(),
                self.get_cli_path(&worker_kind),
            )?;
            
            let worker = Arc::new(Mutex::new(worker));
            self.workers.push(worker.clone());
            
            // Execute the task
            let result = {
                let mut w = worker.lock().await;
                w.execute().await
            };
            
            response.task_results.push(TaskResult {
                task_id: task.id.clone(),
                worker_kind,
                workspace_path: workspace,
                result,
            });
        }
        
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
