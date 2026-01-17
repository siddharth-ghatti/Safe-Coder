//! Live orchestration with real-time streaming from all workers
//!
//! This module provides a live orchestration experience where all worker
//! output is streamed in real-time with proper coordination and display.

use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{Duration, Instant};

use crate::utils::truncate_str;

use crate::orchestrator::{
    streaming_worker::{StreamingWorker, StreamingWorkerStatus, WorkerStreamEvent, StreamingConfig},
    worker::WorkerKind,
    Task, TaskPlan, WorkerState,
};

/// Live orchestration manager with real-time streaming
pub struct LiveOrchestrationManager {
    /// Active streaming workers
    workers: Arc<RwLock<HashMap<String, Arc<Mutex<StreamingWorker>>>>>,
    /// Event aggregator for all workers
    event_aggregator: Arc<Mutex<EventAggregator>>,
    /// Live display manager
    display_manager: Arc<Mutex<LiveDisplayManager>>,
    /// Configuration for streaming
    streaming_config: StreamingConfig,
    /// Base project path
    project_path: PathBuf,
}

/// Aggregates events from all workers
struct EventAggregator {
    /// Events by worker ID
    worker_events: HashMap<String, Vec<WorkerStreamEvent>>,
    /// Global event sender for UI updates
    ui_sender: Option<mpsc::UnboundedSender<OrchestratorEvent>>,
    /// Last activity per worker
    last_activity: HashMap<String, Instant>,
}

/// Events sent to the UI/display layer
#[derive(Debug, Clone)]
pub enum OrchestratorEvent {
    /// Worker started
    WorkerStarted {
        worker_id: String,
        task_id: String,
        worker_kind: WorkerKind,
    },
    /// New output from a worker
    WorkerOutput {
        worker_id: String,
        line: String,
        is_stderr: bool,
        timestamp: Instant,
    },
    /// Worker progress update
    WorkerProgress {
        worker_id: String,
        message: String,
        percentage: Option<f32>,
        timestamp: Instant,
    },
    /// Worker state changed
    WorkerStateChanged {
        worker_id: String,
        new_state: String,
        timestamp: Instant,
    },
    /// Worker completed
    WorkerCompleted {
        worker_id: String,
        success: bool,
        duration: Duration,
        final_output: String,
        timestamp: Instant,
    },
    /// Overall orchestration progress
    OrchestrationProgress {
        completed_workers: usize,
        total_workers: usize,
        active_workers: Vec<String>,
        timestamp: Instant,
    },
    /// Error occurred
    Error {
        worker_id: Option<String>,
        error: String,
        timestamp: Instant,
    },
}

/// Manages live display of orchestration progress
struct LiveDisplayManager {
    /// Terminal width for formatting
    terminal_width: usize,
    /// Show detailed output per worker
    show_detailed_output: bool,
    /// Maximum lines to keep per worker
    max_lines_per_worker: usize,
    /// Color support
    color_enabled: bool,
}

impl LiveOrchestrationManager {
    /// Create a new live orchestration manager
    pub fn new(project_path: PathBuf) -> Self {
        Self {
            workers: Arc::new(RwLock::new(HashMap::new())),
            event_aggregator: Arc::new(Mutex::new(EventAggregator::new())),
            display_manager: Arc::new(Mutex::new(LiveDisplayManager::new())),
            streaming_config: StreamingConfig {
                enabled: true,
                buffer_size: 4096,
                flush_interval_ms: 50, // Very responsive
                max_line_length: 2048,
                progress_detection: true,
                heartbeat_interval_sec: 5, // More frequent heartbeats
            },
            project_path,
        }
    }

    /// Configure streaming settings
    pub fn with_streaming_config(mut self, config: StreamingConfig) -> Self {
        self.streaming_config = config;
        self
    }

    /// Set up UI event stream
    pub fn with_ui_events(self) -> (Self, mpsc::UnboundedReceiver<OrchestratorEvent>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        
        // Set up the UI sender in the event aggregator
        tokio::spawn({
            let aggregator = self.event_aggregator.clone();
            async move {
                let mut agg = aggregator.lock().await;
                agg.ui_sender = Some(sender);
            }
        });

        (self, receiver)
    }

    /// Execute a task plan with live streaming from all workers
    pub async fn execute_plan_live(&self, plan: &TaskPlan) -> Result<Vec<TaskResult>> {
        println!("🚀 Starting Live Orchestration");
        println!("═══════════════════════════════");
        println!("Plan: {}", plan.summary);
        println!("Tasks: {}", plan.tasks.len());
        println!("Streaming: ✓ Enabled");
        println!();

        let mut results = Vec::new();
        let mut active_tasks = Vec::new();

        // Start all workers with streaming
        for task in &plan.tasks {
            let worker_id = format!("worker-{}", task.id);
            let workspace = self.create_workspace(&worker_id).await?;
            
            let worker_kind = task.preferred_worker.clone().unwrap_or(WorkerKind::ClaudeCode);
            let cli_path = self.get_cli_path(&worker_kind);

            let mut streaming_worker = StreamingWorker::new(
                task.clone(),
                workspace,
                worker_kind.clone(),
                cli_path,
            )?
            .with_streaming_config(self.streaming_config.clone());

            let (worker, event_receiver) = streaming_worker.with_event_stream();
            
            // Store the worker
            {
                let mut workers = self.workers.write().await;
                workers.insert(worker_id.clone(), Arc::new(Mutex::new(worker)));
            }

            // Emit worker started event
            {
                let mut aggregator = self.event_aggregator.lock().await;
                aggregator.emit_ui_event(OrchestratorEvent::WorkerStarted {
                    worker_id: worker_id.clone(),
                    task_id: task.id.clone(),
                    worker_kind: worker_kind.clone(),
                }).await;
            }

            // Spawn worker execution
            let worker_handle = {
                let workers = self.workers.clone();
                let worker_id_clone = worker_id.clone();
                
                tokio::spawn(async move {
                    let worker = {
                        let workers_read = workers.read().await;
                        workers_read.get(&worker_id_clone).unwrap().clone()
                    };
                    
                    let mut worker_guard = worker.lock().await;
                    worker_guard.execute_streaming().await
                })
            };

            // Spawn event processor for this worker
            let event_processor = {
                let aggregator = self.event_aggregator.clone();
                let worker_id_clone = worker_id.clone();
                
                tokio::spawn(async move {
                    let mut event_rx = event_receiver;
                    while let Some(event) = event_rx.recv().await {
                        let mut agg = aggregator.lock().await;
                        agg.process_worker_event(worker_id_clone.clone(), event).await;
                    }
                })
            };

            active_tasks.push((worker_id, worker_handle, event_processor));
        }

        // Start the live display
        let display_task = self.start_live_display().await;

        // Wait for all workers to complete
        for (worker_id, worker_handle, event_processor) in active_tasks {
            let result = worker_handle.await?;
            event_processor.abort(); // Stop event processing for this worker

            results.push(TaskResult {
                worker_id,
                result: result.map_err(|e| e.to_string()),
            });
        }

        // Stop live display
        display_task.abort();

        // Final summary
        self.print_final_summary(&results).await;

        Ok(results)
    }

    /// Create workspace for a worker
    async fn create_workspace(&self, worker_id: &str) -> Result<PathBuf> {
        let workspace = self.project_path
            .join(".safe-coder-workspaces")
            .join(worker_id);

        if !workspace.exists() {
            tokio::fs::create_dir_all(&workspace).await?;
        }

        Ok(workspace)
    }

    /// Get CLI path for worker kind
    fn get_cli_path(&self, kind: &WorkerKind) -> String {
        match kind {
            WorkerKind::ClaudeCode => "claude".to_string(),
            WorkerKind::GeminiCli => "gemini".to_string(),
            WorkerKind::SafeCoder => {
                std::env::current_exe()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "safe-coder".to_string())
            }
            WorkerKind::GitHubCopilot => "gh".to_string(),
        }
    }

    /// Start the live display task
    async fn start_live_display(&self) -> tokio::task::JoinHandle<()> {
        let display_manager = self.display_manager.clone();
        let workers = self.workers.clone();

        tokio::spawn(async move {
            let mut update_interval = tokio::time::interval(Duration::from_millis(100));
            
            loop {
                update_interval.tick().await;
                
                let mut display = display_manager.lock().await;
                let workers_read = workers.read().await;
                
                display.update_display(&workers_read).await;
                
                // Check if all workers are done
                let all_done = workers_read.values().all(|worker| {
                    // This is a simplified check - in practice we'd check the actual worker state
                    false // Continue for now
                });
                
                if all_done {
                    break;
                }
            }
        })
    }

    /// Print final orchestration summary
    async fn print_final_summary(&self, results: &[TaskResult]) {
        println!("\n🎯 Orchestration Complete");
        println!("═══════════════════════════");
        
        let successful = results.iter().filter(|r| r.result.is_ok()).count();
        let failed = results.len() - successful;
        
        println!("✓ Successful: {}", successful);
        if failed > 0 {
            println!("✗ Failed: {}", failed);
        }
        println!();

        for result in results {
            let status = if result.result.is_ok() { "✓" } else { "✗" };
            println!("  {} {}", status, result.worker_id);
            
            if let Err(error) = &result.result {
                println!("    Error: {}", error);
            }
        }
    }

    /// Get status of all workers
    pub async fn get_worker_statuses(&self) -> Vec<StreamingWorkerStatus> {
        let workers = self.workers.read().await;
        let mut statuses = Vec::new();

        for worker in workers.values() {
            let worker_guard = worker.lock().await;
            statuses.push(worker_guard.status());
        }

        statuses
    }

    /// Stop all workers
    pub async fn stop_all_workers(&self) -> Result<()> {
        let workers = self.workers.write().await;
        
        for worker in workers.values() {
            let mut worker_guard = worker.lock().await;
            worker_guard.cancel().await?;
        }

        Ok(())
    }
}

impl EventAggregator {
    fn new() -> Self {
        Self {
            worker_events: HashMap::new(),
            ui_sender: None,
            last_activity: HashMap::new(),
        }
    }

    async fn process_worker_event(&mut self, worker_id: String, event: WorkerStreamEvent) {
        // Store the event
        self.worker_events.entry(worker_id.clone()).or_default().push(event.clone());
        self.last_activity.insert(worker_id.clone(), Instant::now());

        // Convert to UI event and emit
        if let Some(ui_event) = self.convert_to_ui_event(worker_id, event).await {
            self.emit_ui_event(ui_event).await;
        }
    }

    async fn convert_to_ui_event(&self, worker_id: String, event: WorkerStreamEvent) -> Option<OrchestratorEvent> {
        match event {
            WorkerStreamEvent::Output { line, timestamp, is_stderr } => {
                Some(OrchestratorEvent::WorkerOutput {
                    worker_id,
                    line,
                    is_stderr,
                    timestamp,
                })
            }
            WorkerStreamEvent::Progress { message, percentage, timestamp } => {
                Some(OrchestratorEvent::WorkerProgress {
                    worker_id,
                    message,
                    percentage,
                    timestamp,
                })
            }
            WorkerStreamEvent::StateChanged { new_state, timestamp, .. } => {
                Some(OrchestratorEvent::WorkerStateChanged {
                    worker_id,
                    new_state: format!("{:?}", new_state),
                    timestamp,
                })
            }
            WorkerStreamEvent::Completed { success, final_output, duration, timestamp } => {
                Some(OrchestratorEvent::WorkerCompleted {
                    worker_id,
                    success,
                    duration,
                    final_output,
                    timestamp,
                })
            }
        }
    }

    async fn emit_ui_event(&self, event: OrchestratorEvent) {
        if let Some(ref sender) = self.ui_sender {
            let _ = sender.send(event);
        }
    }
}

impl LiveDisplayManager {
    fn new() -> Self {
        Self {
            terminal_width: 80, // Default, should be detected from terminal
            show_detailed_output: true,
            max_lines_per_worker: 10,
            color_enabled: atty::is(atty::Stream::Stdout),
        }
    }

    async fn update_display(&mut self, workers: &HashMap<String, Arc<Mutex<StreamingWorker>>>) {
        // Clear screen and move to top (for live updates)
        if self.color_enabled {
            print!("\x1B[2J\x1B[H"); // Clear screen and move cursor to top
        } else {
            println!("\n{}", "═".repeat(self.terminal_width));
        }

        println!("🔄 Live Orchestration Status");
        println!("{}", "─".repeat(self.terminal_width));

        let mut active_count = 0;
        let mut completed_count = 0;
        let mut failed_count = 0;

        for (worker_id, worker) in workers {
            let worker_guard = worker.lock().await;
            let status = worker_guard.status();

            match status.state {
                WorkerState::Running => {
                    active_count += 1;
                    self.print_worker_status(worker_id, &status, "🔄").await;
                }
                WorkerState::Completed => {
                    completed_count += 1;
                    self.print_worker_status(worker_id, &status, "✅").await;
                }
                WorkerState::Failed(_) => {
                    failed_count += 1;
                    self.print_worker_status(worker_id, &status, "❌").await;
                }
                _ => {
                    self.print_worker_status(worker_id, &status, "⏳").await;
                }
            }
        }

        println!("\n📊 Summary: {} active, {} completed, {} failed", 
                 active_count, completed_count, failed_count);
        
        if self.color_enabled {
            // Move cursor to a safe position and flush
            print!("\x1B[999;1H");
            use std::io::{self, Write};
            io::stdout().flush().unwrap();
        }
    }

    async fn print_worker_status(&self, worker_id: &str, status: &StreamingWorkerStatus, icon: &str) {
        let elapsed = status.started_at
            .map(|start| format!("{:.1}s", Instant::now().duration_since(start).as_secs_f64()))
            .unwrap_or_else(|| "-".to_string());

        println!("{} {} | Lines: {} | Elapsed: {} | {:?}", 
                 icon, worker_id, status.lines_processed, elapsed, status.kind);

        if self.show_detailed_output && !status.output.is_empty() {
            let recent_lines: Vec<&str> = status.output
                .lines()
                .rev()
                .take(3) // Show last 3 lines
                .collect();

            for line in recent_lines.iter().rev() {
                if line.trim().len() > 0 {
                    let truncated = if line.chars().count() > 60 {
                        format!("{}...", truncate_str(line, 57))
                    } else {
                        line.to_string()
                    };
                    println!("    💬 {}", truncated);
                }
            }
        }
    }
}

/// Result of a task execution
#[derive(Debug, Clone)]
pub struct TaskResult {
    /// Worker identifier
    pub worker_id: String,
    /// Execution result
    pub result: Result<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_live_orchestration_manager_creation() {
        let temp_dir = tempdir().unwrap();
        let manager = LiveOrchestrationManager::new(temp_dir.path().to_path_buf());
        
        assert!(manager.streaming_config.enabled);
        assert_eq!(manager.streaming_config.flush_interval_ms, 50);
    }

    #[tokio::test]
    async fn test_event_aggregator() {
        let mut aggregator = EventAggregator::new();
        let worker_id = "test-worker".to_string();
        
        let event = WorkerStreamEvent::Output {
            line: "Test output".to_string(),
            timestamp: Instant::now(),
            is_stderr: false,
        };

        aggregator.process_worker_event(worker_id.clone(), event).await;
        
        assert!(aggregator.worker_events.contains_key(&worker_id));
        assert_eq!(aggregator.worker_events[&worker_id].len(), 1);
    }
}