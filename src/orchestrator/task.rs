//! Task definitions for the orchestrator

use serde::{Deserialize, Serialize};
use crate::orchestrator::WorkerKind;

/// A single task to be executed by a worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier for this task
    pub id: String,
    /// Human-readable description
    pub description: String,
    /// Detailed instructions for the worker
    pub instructions: String,
    /// Files that this task may need to modify
    pub relevant_files: Vec<String>,
    /// Dependencies on other tasks (by id)
    pub dependencies: Vec<String>,
    /// Preferred worker for this task
    pub preferred_worker: Option<WorkerKind>,
    /// Priority (lower = higher priority)
    pub priority: u32,
    /// Current status of the task
    pub status: TaskStatus,
}

/// Status of a task
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TaskStatus {
    /// Task is waiting to be executed
    Pending,
    /// Task is currently being executed
    Running,
    /// Task completed successfully
    Completed,
    /// Task failed
    Failed(String),
    /// Task was cancelled
    Cancelled,
}

impl Default for TaskStatus {
    fn default() -> Self {
        TaskStatus::Pending
    }
}

/// A plan consisting of multiple tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    /// Unique identifier for this plan
    pub id: String,
    /// Original user request
    pub original_request: String,
    /// High-level summary of the plan
    pub summary: String,
    /// Individual tasks to execute
    pub tasks: Vec<Task>,
    /// Execution order (task ids in order)
    pub execution_order: Vec<String>,
}

impl TaskPlan {
    /// Create a new task plan
    pub fn new(id: String, request: String, summary: String) -> Self {
        Self {
            id,
            original_request: request,
            summary,
            tasks: Vec::new(),
            execution_order: Vec::new(),
        }
    }
    
    /// Add a task to the plan
    pub fn add_task(&mut self, task: Task) {
        self.execution_order.push(task.id.clone());
        self.tasks.push(task);
    }
    
    /// Get tasks in execution order
    pub fn tasks_in_order(&self) -> Vec<&Task> {
        self.execution_order.iter()
            .filter_map(|id| self.tasks.iter().find(|t| &t.id == id))
            .collect()
    }
    
    /// Check if all tasks are complete
    pub fn is_complete(&self) -> bool {
        self.tasks.iter().all(|t| matches!(t.status, TaskStatus::Completed))
    }
    
    /// Check if any task failed
    pub fn has_failures(&self) -> bool {
        self.tasks.iter().any(|t| matches!(t.status, TaskStatus::Failed(_)))
    }
}

impl Task {
    /// Create a new task
    pub fn new(id: String, description: String, instructions: String) -> Self {
        Self {
            id,
            description,
            instructions,
            relevant_files: Vec::new(),
            dependencies: Vec::new(),
            preferred_worker: None,
            priority: 0,
            status: TaskStatus::default(),
        }
    }
    
    /// Add relevant files
    pub fn with_files(mut self, files: Vec<String>) -> Self {
        self.relevant_files = files;
        self
    }
    
    /// Add dependencies
    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.dependencies = deps;
        self
    }
    
    /// Set preferred worker
    pub fn with_worker(mut self, worker: WorkerKind) -> Self {
        self.preferred_worker = Some(worker);
        self
    }
    
    /// Set priority
    pub fn with_priority(mut self, priority: u32) -> Self {
        self.priority = priority;
        self
    }
}
