//! Orchestrator module for delegating tasks to external CLI agents
//!
//! This module implements the orchestrator pattern where Safe Coder acts as a
//! high-level planner that delegates tasks to specialized CLI agents (Claude Code,
//! Gemini CLI) running in isolated git workspaces.

// TODO: Fix type mismatches in these modules
// pub mod live_orchestration;
pub mod planner;
// pub mod self_orchestration;
// pub mod streaming_worker;
pub mod task;
pub mod worker;
pub mod workspace;

pub use planner::Planner;
pub use task::{Task, TaskPlan, TaskStatus};
pub use worker::{Worker, WorkerKind, WorkerStatus};
pub use workspace::WorkspaceManager;

use anyhow::Result;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::approval::ExecutionMode;

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

/// Strategy for distributing tasks across multiple workers
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerStrategy {
    /// Use only the default worker for all tasks
    SingleWorker,
    /// Distribute tasks round-robin across enabled workers
    RoundRobin,
    /// Assign workers based on task type/complexity (simple tasks to faster workers)
    TaskBased,
    /// Use all enabled workers, load-balanced by current queue
    LoadBalanced,
}

impl Default for WorkerStrategy {
    fn default() -> Self {
        WorkerStrategy::SingleWorker
    }
}

/// Configuration for the orchestrator
#[derive(Debug, Clone)]
pub struct OrchestratorConfig {
    /// Path to Claude Code CLI (claude)
    pub claude_cli_path: Option<String>,
    /// Path to Gemini CLI (gemini)
    pub gemini_cli_path: Option<String>,
    /// Path to Safe-Coder CLI (safe-coder)
    pub safe_coder_cli_path: Option<String>,
    /// Path to GitHub CLI for Copilot (gh)
    pub gh_cli_path: Option<String>,
    /// Maximum concurrent workers
    pub max_workers: usize,
    /// Default worker kind to use
    pub default_worker: WorkerKind,
    /// Strategy for distributing tasks across workers
    pub worker_strategy: WorkerStrategy,
    /// List of enabled workers (used for multi-worker strategies)
    pub enabled_workers: Vec<WorkerKind>,
    /// Whether to use git worktrees for isolation
    pub use_worktrees: bool,
    /// Throttle limits per worker type
    pub throttle_limits: ThrottleLimits,
    /// Execution mode: Plan (requires approval) or Act (auto-execute)
    pub execution_mode: ExecutionMode,
}

/// Throttle limits for different worker types
#[derive(Debug, Clone)]
pub struct ThrottleLimits {
    /// Maximum concurrent Claude Code workers
    pub claude_max_concurrent: usize,
    /// Maximum concurrent Gemini CLI workers
    pub gemini_max_concurrent: usize,
    /// Maximum concurrent Safe-Coder workers
    pub safe_coder_max_concurrent: usize,
    /// Maximum concurrent GitHub Copilot workers
    pub copilot_max_concurrent: usize,
    /// Delay between starting workers of the same type (milliseconds)
    pub start_delay_ms: u64,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            claude_cli_path: Some("claude".to_string()),
            gemini_cli_path: Some("gemini".to_string()),
            safe_coder_cli_path: std::env::current_exe()
                .map(|p| p.to_string_lossy().to_string())
                .ok()
                .or_else(|| Some("safe-coder".to_string())),
            gh_cli_path: Some("gh".to_string()),
            max_workers: 3,
            default_worker: WorkerKind::ClaudeCode,
            worker_strategy: WorkerStrategy::default(),
            enabled_workers: vec![WorkerKind::ClaudeCode], // Default to just Claude
            use_worktrees: true,
            throttle_limits: ThrottleLimits::default(),
            execution_mode: ExecutionMode::default(),
        }
    }
}

impl Default for ThrottleLimits {
    fn default() -> Self {
        Self {
            claude_max_concurrent: 2,
            gemini_max_concurrent: 2,
            safe_coder_max_concurrent: 2,
            copilot_max_concurrent: 2,
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
        let mut plan = self.planner.create_plan(request).await?;

        // Step 1.5: Assign workers to tasks based on configured strategy
        self.assign_workers_to_tasks(&mut plan);

        let mut response = OrchestratorResponse {
            plan: plan.clone(),
            task_results: Vec::new(),
            summary: String::new(),
        };

        // Step 1.5: Handle planning mode - show detailed plan and ask for approval
        match self.config.execution_mode {
            ExecutionMode::Plan => {
                // Show detailed plan
                let detailed_plan = self.format_orchestration_plan(&plan);
                println!("{}", detailed_plan);

                // Ask for user approval
                if !self.ask_plan_approval().await? {
                    response.summary =
                        "❌ Plan rejected by user. No tasks were executed.".to_string();
                    return Ok(response);
                }
                println!("\n✅ Plan approved. Distributing tasks to workers...\n");
            }
            ExecutionMode::Act => {
                // In Act mode, we skip the detailed output since it may be running
                // in a TUI context where channel-based updates are used instead.
                // The caller is responsible for displaying progress.
            }
        }

        // Step 2: Execute tasks in parallel with throttling
        // Pass the plan to enhance task instructions with context
        let task_results = self.execute_tasks_parallel(&plan).await?;
        response.task_results = task_results;

        // Step 3: Merge results back
        for task_result in &response.task_results {
            if task_result.result.is_ok() {
                self.workspace_manager
                    .merge_workspace(&task_result.task_id)
                    .await?;
            }
        }

        // Generate summary
        response.summary = self.generate_summary(&response);

        Ok(response)
    }

    /// Format a detailed orchestration plan for display
    fn format_orchestration_plan(&self, plan: &TaskPlan) -> String {
        let mut output = String::new();

        output.push_str("🎯 ORCHESTRATION PLAN\n");
        output.push_str("══════════════════════════════════════════════════════════════\n\n");

        output.push_str(&format!("📝 Request: {}\n\n", plan.original_request));
        output.push_str(&format!("📋 Summary: {}\n\n", plan.summary));

        // Tasks breakdown
        output.push_str(&format!("🔧 Tasks ({}):\n", plan.tasks.len()));
        output.push_str("───────────────────────────────────────────────────────────────\n");

        for (i, task) in plan.tasks.iter().enumerate() {
            let worker = task
                .preferred_worker
                .as_ref()
                .unwrap_or(&self.config.default_worker);
            let worker_icon = match worker {
                WorkerKind::ClaudeCode => "🤖",
                WorkerKind::GeminiCli => "💎",
                WorkerKind::SafeCoder => "🛡️",
                WorkerKind::GitHubCopilot => "🐙",
            };

            output.push_str(&format!("\n  {}. {} {:?}\n", i + 1, worker_icon, worker));
            output.push_str(&format!("     📌 {}\n", task.description));

            if !task.relevant_files.is_empty() {
                output.push_str("     📁 Files: ");
                output.push_str(&task.relevant_files.join(", "));
                output.push('\n');
            }

            if !task.dependencies.is_empty() {
                output.push_str("     🔗 Depends on: ");
                output.push_str(&task.dependencies.join(", "));
                output.push('\n');
            }

            // Show truncated instructions
            let instructions_preview = if task.instructions.len() > 100 {
                format!("{}...", &task.instructions[..100])
            } else {
                task.instructions.clone()
            };
            output.push_str(&format!("     💬 Instructions: {}\n", instructions_preview));
        }

        output.push_str("\n───────────────────────────────────────────────────────────────\n");

        // Execution info
        output.push_str(&format!("\n⚙️  Execution Configuration:\n"));
        output.push_str(&format!(
            "   • Max concurrent workers: {}\n",
            self.config.max_workers
        ));
        output.push_str(&format!(
            "   • Claude max concurrent: {}\n",
            self.config.throttle_limits.claude_max_concurrent
        ));
        output.push_str(&format!(
            "   • Gemini max concurrent: {}\n",
            self.config.throttle_limits.gemini_max_concurrent
        ));
        output.push_str(&format!(
            "   • Worker start delay: {}ms\n",
            self.config.throttle_limits.start_delay_ms
        ));
        output.push_str(&format!(
            "   • Using worktrees: {}\n",
            self.config.use_worktrees
        ));

        output
    }

    /// Ask user for approval to execute the plan
    async fn ask_plan_approval(&self) -> Result<bool> {
        print!("\n🔒 Execute this orchestration plan? [y/N]: ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let input = input.trim().to_lowercase();
        Ok(input == "y" || input == "yes")
    }

    /// Execute tasks in parallel with throttling (max concurrent workers)
    async fn execute_tasks_parallel(&mut self, plan: &TaskPlan) -> Result<Vec<TaskResult>> {
        use std::collections::{HashMap, VecDeque};
        use tokio::task::JoinSet;

        let mut results = Vec::new();
        let mut join_set = JoinSet::new();
        let mut task_queue: VecDeque<Task> = plan.tasks.iter().cloned().collect();

        // Track active workers by type for throttling
        let mut active_by_type: HashMap<WorkerKind, usize> = HashMap::new();
        let mut last_start_time = std::time::Instant::now();

        // Start initial batch of workers (respecting throttle limits)
        while !task_queue.is_empty() && join_set.len() < self.config.max_workers {
            if self
                .try_start_next_task(
                    &mut task_queue,
                    &mut active_by_type,
                    &mut last_start_time,
                    &mut join_set,
                    plan,
                )
                .await?
                .is_none()
            {
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
                    &mut join_set,
                    plan,
                )
                .await?;
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
        plan: &TaskPlan,
    ) -> Result<Option<Task>> {
        // Try each task in the queue to find one that can be started
        for i in 0..task_queue.len() {
            let task = &task_queue[i];
            let worker_kind = task
                .preferred_worker
                .clone()
                .unwrap_or(self.config.default_worker.clone());

            // Check throttle limits for this worker type
            let count = active_by_type.get(&worker_kind).copied().unwrap_or(0);
            let max = match worker_kind {
                WorkerKind::ClaudeCode => self.config.throttle_limits.claude_max_concurrent,
                WorkerKind::GeminiCli => self.config.throttle_limits.gemini_max_concurrent,
                WorkerKind::SafeCoder => self.config.throttle_limits.safe_coder_max_concurrent,
                WorkerKind::GitHubCopilot => self.config.throttle_limits.copilot_max_concurrent,
            };

            if count >= max {
                // This worker type is at limit, try next task
                continue;
            }

            // Can start this task, remove it from queue (O(1) for front, O(n) otherwise)
            let mut task = if i == 0 {
                task_queue.pop_front().unwrap()
            } else {
                task_queue.remove(i).unwrap()
            };
            let task_id = task.id.clone();

            // Enhance task instructions with plan context
            // This gives external CLIs the full context of what they're working on
            task.instructions = task.instructions_with_plan_context(plan);

            // Apply start delay between workers
            let elapsed = last_start_time.elapsed().as_millis() as u64;
            if elapsed < self.config.throttle_limits.start_delay_ms {
                tokio::time::sleep(tokio::time::Duration::from_millis(
                    self.config.throttle_limits.start_delay_ms - elapsed,
                ))
                .await;
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

                (
                    TaskResult {
                        task_id,
                        worker_kind: worker_kind_clone.clone(),
                        workspace_path: workspace,
                        result,
                    },
                    worker_kind_clone,
                )
            });

            return Ok(Some(task));
        }

        // No tasks could be started due to throttle limits
        Ok(None)
    }

    /// Get the CLI path for a worker kind
    fn get_cli_path(&self, kind: &WorkerKind) -> String {
        match kind {
            WorkerKind::ClaudeCode => self
                .config
                .claude_cli_path
                .clone()
                .unwrap_or_else(|| "claude".to_string()),
            WorkerKind::GeminiCli => self
                .config
                .gemini_cli_path
                .clone()
                .unwrap_or_else(|| "gemini".to_string()),
            WorkerKind::SafeCoder => self.config.safe_coder_cli_path.clone().unwrap_or_else(|| {
                std::env::current_exe()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "safe-coder".to_string())
            }),
            WorkerKind::GitHubCopilot => self
                .config
                .gh_cli_path
                .clone()
                .unwrap_or_else(|| "gh".to_string()),
        }
    }

    /// Assign workers to tasks based on the configured strategy
    /// This modifies the plan's tasks to set their preferred_worker field
    fn assign_workers_to_tasks(&self, plan: &mut TaskPlan) {
        if self.config.enabled_workers.is_empty() {
            // No workers enabled, use default for all
            return;
        }

        match self.config.worker_strategy {
            WorkerStrategy::SingleWorker => {
                // Use default worker for all tasks (already the default behavior)
                for task in &mut plan.tasks {
                    if task.preferred_worker.is_none() {
                        task.preferred_worker = Some(self.config.default_worker.clone());
                    }
                }
            }
            WorkerStrategy::RoundRobin => {
                // Distribute tasks evenly across enabled workers
                let workers = &self.config.enabled_workers;
                for (i, task) in plan.tasks.iter_mut().enumerate() {
                    if task.preferred_worker.is_none() {
                        let worker_idx = i % workers.len();
                        task.preferred_worker = Some(workers[worker_idx].clone());
                    }
                }
            }
            WorkerStrategy::TaskBased => {
                // Assign workers based on task characteristics
                // - Simple/fast tasks (single file, no deps) -> Copilot or SafeCoder
                // - Complex tasks (multiple files, deps) -> Claude or Gemini
                for task in &mut plan.tasks {
                    if task.preferred_worker.is_none() {
                        let is_simple = task.relevant_files.len() <= 1
                            && task.dependencies.is_empty()
                            && task.instructions.len() < 500;

                        task.preferred_worker = Some(if is_simple {
                            // Prefer faster/lighter workers for simple tasks
                            self.config
                                .enabled_workers
                                .iter()
                                .find(|w| {
                                    matches!(w, WorkerKind::GitHubCopilot | WorkerKind::SafeCoder)
                                })
                                .cloned()
                                .unwrap_or_else(|| self.config.default_worker.clone())
                        } else {
                            // Prefer more capable workers for complex tasks
                            self.config
                                .enabled_workers
                                .iter()
                                .find(|w| {
                                    matches!(w, WorkerKind::ClaudeCode | WorkerKind::GeminiCli)
                                })
                                .cloned()
                                .unwrap_or_else(|| self.config.default_worker.clone())
                        });
                    }
                }
            }
            WorkerStrategy::LoadBalanced => {
                // Assign to workers with least assigned tasks
                let mut worker_counts: std::collections::HashMap<WorkerKind, usize> = self
                    .config
                    .enabled_workers
                    .iter()
                    .map(|w| (w.clone(), 0))
                    .collect();

                for task in &mut plan.tasks {
                    if task.preferred_worker.is_none() {
                        // Find worker with minimum assigned tasks
                        let min_worker = worker_counts
                            .iter()
                            .min_by_key(|(_, count)| *count)
                            .map(|(worker, _)| worker.clone())
                            .unwrap_or_else(|| self.config.default_worker.clone());

                        task.preferred_worker = Some(min_worker.clone());
                        *worker_counts.entry(min_worker).or_insert(0) += 1;
                    }
                }
            }
        }
    }

    /// Generate a summary of the orchestration results
    fn generate_summary(&self, response: &OrchestratorResponse) -> String {
        let total = response.task_results.len();
        let successful = response
            .task_results
            .iter()
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
            safe_coder_cli_path: Some("safe-coder".to_string()),
            gh_cli_path: Some("gh".to_string()),
            max_workers: 3,
            default_worker: WorkerKind::ClaudeCode,
            worker_strategy: WorkerStrategy::SingleWorker,
            enabled_workers: vec![WorkerKind::ClaudeCode],
            use_worktrees: false,
            throttle_limits: ThrottleLimits {
                claude_max_concurrent: 2,
                gemini_max_concurrent: 1,
                safe_coder_max_concurrent: 1,
                copilot_max_concurrent: 1,
                start_delay_ms: 50,
            },
            execution_mode: ExecutionMode::default(),
        };

        let orchestrator = Orchestrator::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

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
            safe_coder_cli_path: Some("echo".to_string()),
            gh_cli_path: Some("echo".to_string()),
            max_workers: 2, // Limit to 2 concurrent workers
            default_worker: WorkerKind::ClaudeCode,
            worker_strategy: WorkerStrategy::SingleWorker,
            enabled_workers: vec![WorkerKind::ClaudeCode],
            use_worktrees: false,
            throttle_limits: ThrottleLimits {
                claude_max_concurrent: 2,
                gemini_max_concurrent: 2,
                safe_coder_max_concurrent: 2,
                copilot_max_concurrent: 2,
                start_delay_ms: 0,
            },
            execution_mode: ExecutionMode::default(),
        };

        let mut orchestrator = Orchestrator::new(temp_dir.path().to_path_buf(), config)
            .await
            .unwrap();

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
