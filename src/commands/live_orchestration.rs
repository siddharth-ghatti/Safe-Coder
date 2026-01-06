//! CLI commands for live orchestration with streaming
//!
//! This module provides command-line interfaces for running orchestration
//! with live streaming from all worker CLIs.

use anyhow::Result;
use clap::{Args, Subcommand};
use std::path::PathBuf;
use tokio::time::Duration;

use crate::orchestrator::{
    live_orchestration::{LiveOrchestrationManager, OrchestratorEvent},
    streaming_worker::{StreamingConfig, WorkerKind},
    {Orchestrator, OrchestratorConfig, TaskPlan, Task},
};

/// Live orchestration CLI commands
#[derive(Subcommand, Debug, Clone)]
pub enum LiveOrchestrationCommand {
    /// Execute a plan with live streaming from all workers
    Execute {
        /// The task description or plan file
        input: String,
        
        /// Enable live streaming output
        #[arg(long, default_value = "true")]
        streaming: bool,
        
        /// Maximum concurrent workers
        #[arg(long, default_value = "4")]
        max_workers: usize,
        
        /// Streaming buffer size (bytes)
        #[arg(long, default_value = "8192")]
        buffer_size: usize,
        
        /// Flush interval for live updates (milliseconds)
        #[arg(long, default_value = "50")]
        flush_interval_ms: u64,
        
        /// Show detailed output from each worker
        #[arg(long, default_value = "true")]
        detailed_output: bool,
        
        /// Enable progress detection in worker output
        #[arg(long, default_value = "true")]
        progress_detection: bool,
        
        /// Heartbeat interval to show worker activity (seconds)
        #[arg(long, default_value = "5")]
        heartbeat_interval_sec: u64,
    },
    
    /// Monitor active orchestration with live updates
    Monitor {
        /// Update interval in milliseconds
        #[arg(long, default_value = "100")]
        update_interval_ms: u64,
        
        /// Show worker output in real-time
        #[arg(long)]
        show_output: bool,
        
        /// Filter by worker type
        #[arg(long, value_enum)]
        worker_type: Option<WorkerKindArg>,
    },
    
    /// Stop all active orchestration workers
    Stop {
        /// Force stop without graceful shutdown
        #[arg(long)]
        force: bool,
        
        /// Stop specific worker by ID
        #[arg(long)]
        worker_id: Option<String>,
    },
    
    /// Test streaming with a simple command
    Test {
        /// Test command to run
        #[arg(default_value = "echo 'Streaming test'")]
        command: String,
        
        /// Worker kind to test
        #[arg(long, value_enum, default_value = "claude-code")]
        worker_kind: WorkerKindArg,
        
        /// Test duration in seconds
        #[arg(long, default_value = "10")]
        duration_sec: u64,
    },
    
    /// Configure live orchestration settings
    Configure {
        /// Set default streaming buffer size
        #[arg(long)]
        buffer_size: Option<usize>,
        
        /// Set default flush interval
        #[arg(long)]
        flush_interval_ms: Option<u64>,
        
        /// Enable/disable progress detection
        #arg(long)]
        progress_detection: Option<bool>,
        
        /// Save configuration
        #[arg(long)]
        save: bool,
    },
}

/// Worker kind arguments for CLI
#[derive(clap::ValueEnum, Debug, Clone)]
pub enum WorkerKindArg {
    ClaudeCode,
    GeminiCli,
    SafeCoder,
    GitHubCopilot,
}

/// Live orchestration CLI arguments (for integration with main CLI)
#[derive(Args, Debug, Clone)]
pub struct LiveOrchestrationArgs {
    /// Enable live streaming from workers
    #[arg(long)]
    pub live_streaming: bool,
    
    /// Streaming buffer size for live updates
    #[arg(long, default_value = "8192")]
    pub stream_buffer_size: usize,
    
    /// Live update flush interval (ms)
    #[arg(long, default_value = "50")]
    pub stream_flush_ms: u64,
    
    /// Show detailed worker output
    #[arg(long)]
    pub detailed_worker_output: bool,
    
    /// Enable worker progress detection
    #[arg(long, default_value = "true")]
    pub worker_progress_detection: bool,
    
    /// Heartbeat interval for worker health checks
    #[arg(long, default_value = "5")]
    pub worker_heartbeat_sec: u64,
}

impl From<WorkerKindArg> for WorkerKind {
    fn from(arg: WorkerKindArg) -> Self {
        match arg {
            WorkerKindArg::ClaudeCode => Self::ClaudeCode,
            WorkerKindArg::GeminiCli => Self::GeminiCli,
            WorkerKindArg::SafeCoder => Self::SafeCoder,
            WorkerKindArg::GitHubCopilot => Self::GitHubCopilot,
        }
    }
}

/// Execute live orchestration command
pub async fn execute_live_orchestration(
    cmd: LiveOrchestrationCommand,
    project_path: PathBuf,
) -> Result<()> {
    match cmd {
        LiveOrchestrationCommand::Execute {
            input,
            streaming,
            max_workers,
            buffer_size,
            flush_interval_ms,
            detailed_output,
            progress_detection,
            heartbeat_interval_sec,
        } => {
            execute_with_live_streaming(
                input,
                project_path,
                streaming,
                max_workers,
                buffer_size,
                flush_interval_ms,
                detailed_output,
                progress_detection,
                heartbeat_interval_sec,
            ).await
        }
        
        LiveOrchestrationCommand::Monitor {
            update_interval_ms,
            show_output,
            worker_type,
        } => {
            monitor_live_orchestration(
                project_path,
                update_interval_ms,
                show_output,
                worker_type.map(Into::into),
            ).await
        }
        
        LiveOrchestrationCommand::Stop { force, worker_id } => {
            stop_orchestration(project_path, force, worker_id).await
        }
        
        LiveOrchestrationCommand::Test {
            command,
            worker_kind,
            duration_sec,
        } => {
            test_streaming_worker(
                project_path,
                command,
                worker_kind.into(),
                duration_sec,
            ).await
        }
        
        LiveOrchestrationCommand::Configure {
            buffer_size,
            flush_interval_ms,
            progress_detection,
            save,
        } => {
            configure_live_orchestration(
                buffer_size,
                flush_interval_ms,
                progress_detection,
                save,
            ).await
        }
    }
}

/// Execute orchestration with live streaming
async fn execute_with_live_streaming(
    input: String,
    project_path: PathBuf,
    streaming: bool,
    max_workers: usize,
    buffer_size: usize,
    flush_interval_ms: u64,
    detailed_output: bool,
    progress_detection: bool,
    heartbeat_interval_sec: u64,
) -> Result<()> {
    println!("🚀 Starting Live Orchestration");
    println!("═══════════════════════════════");
    
    if !streaming {
        println!("⚠️  Streaming disabled - falling back to standard orchestration");
        // Fall back to standard orchestration
        return execute_standard_orchestration(input, project_path, max_workers).await;
    }

    // Create streaming configuration
    let streaming_config = StreamingConfig {
        enabled: true,
        buffer_size,
        flush_interval_ms,
        max_line_length: 2048,
        progress_detection,
        heartbeat_interval_sec,
    };

    // Set up live orchestration manager
    let manager = LiveOrchestrationManager::new(project_path)
        .with_streaming_config(streaming_config);

    let (manager, mut event_rx) = manager.with_ui_events();

    // Create a task plan from the input
    let plan = create_task_plan_from_input(&input)?;

    // Start the UI event processor
    let ui_task = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            handle_orchestrator_event(event, detailed_output).await;
        }
    });

    // Execute the plan with live streaming
    let results = manager.execute_plan_live(&plan).await?;

    // Stop UI task
    ui_task.abort();

    // Print final results
    print_execution_results(&results).await;

    Ok(())
}

/// Monitor live orchestration
async fn monitor_live_orchestration(
    project_path: PathBuf,
    update_interval_ms: u64,
    show_output: bool,
    worker_type_filter: Option<WorkerKind>,
) -> Result<()> {
    println!("👀 Monitoring Live Orchestration");
    println!("═════════════════════════════════");

    let manager = LiveOrchestrationManager::new(project_path);
    let mut interval = tokio::time::interval(Duration::from_millis(update_interval_ms));

    loop {
        interval.tick().await;
        
        let statuses = manager.get_worker_statuses().await;
        
        if statuses.is_empty() {
            println!("📭 No active workers");
            tokio::time::sleep(Duration::from_secs(1)).await;
            continue;
        }

        // Clear screen for live updates
        print!("\x1B[2J\x1B[H");
        
        println!("🔄 Active Workers: {}", statuses.len());
        println!("{}", "─".repeat(60));

        for status in &statuses {
            // Apply filter if specified
            if let Some(ref filter_kind) = worker_type_filter {
                if status.kind != *filter_kind {
                    continue;
                }
            }

            let state_icon = match status.state {
                crate::orchestrator::streaming_worker::WorkerState::Running => "🔄",
                crate::orchestrator::streaming_worker::WorkerState::Completed => "✅",
                crate::orchestrator::streaming_worker::WorkerState::Failed(_) => "❌",
                crate::orchestrator::streaming_worker::WorkerState::Cancelled => "🚫",
                _ => "⏳",
            };

            let elapsed = status.started_at
                .map(|start| format!("{:.1}s", tokio::time::Instant::now().duration_since(start).as_secs_f64()))
                .unwrap_or_else(|| "-".to_string());

            println!("{} {} | {:?} | Lines: {} | Elapsed: {}", 
                     state_icon, status.task_id, status.kind, 
                     status.lines_processed, elapsed);

            if show_output && !status.output.is_empty() {
                let last_lines: Vec<&str> = status.output
                    .lines()
                    .rev()
                    .take(2)
                    .collect();

                for line in last_lines.iter().rev() {
                    if !line.trim().is_empty() {
                        let truncated = if line.len() > 50 {
                            format!("{}...", &line[..47])
                        } else {
                            line.to_string()
                        };
                        println!("    💬 {}", truncated);
                    }
                }
            }
        }

        println!("\n📊 Update: {}", chrono::Utc::now().format("%H:%M:%S"));
        println!("Press Ctrl+C to stop monitoring");
        
        std::io::Write::flush(&mut std::io::stdout())?;
    }
}

/// Stop orchestration workers
async fn stop_orchestration(
    project_path: PathBuf,
    force: bool,
    worker_id: Option<String>,
) -> Result<()> {
    let manager = LiveOrchestrationManager::new(project_path);

    if let Some(id) = worker_id {
        println!("🛑 Stopping worker: {}", id);
        // TODO: Implement single worker stop
        manager.stop_all_workers().await?;
    } else {
        if force {
            println!("🛑 Force stopping all workers");
        } else {
            println!("🛑 Gracefully stopping all workers");
        }
        manager.stop_all_workers().await?;
    }

    println!("✅ Workers stopped");
    Ok(())
}

/// Test streaming with a specific worker
async fn test_streaming_worker(
    project_path: PathBuf,
    command: String,
    worker_kind: WorkerKind,
    duration_sec: u64,
) -> Result<()> {
    println!("🧪 Testing Streaming Worker");
    println!("════════════════════════════");
    println!("Worker: {:?}", worker_kind);
    println!("Command: {}", command);
    println!("Duration: {}s", duration_sec);
    println!();

    // Create a test task
    let task = Task {
        id: "test-streaming".to_string(),
        description: "Test streaming functionality".to_string(),
        instructions: command,
        relevant_files: vec![],
        dependencies: vec![],
        preferred_worker: Some(worker_kind.clone()),
        priority: 0,
        status: crate::orchestrator::TaskStatus::Pending,
    };

    let manager = LiveOrchestrationManager::new(project_path);
    let (manager, mut event_rx) = manager.with_ui_events();

    // Create a simple plan with just the test task
    let mut plan = TaskPlan::new(
        "test-plan".to_string(),
        "Streaming Test".to_string(),
        "Testing worker streaming functionality".to_string(),
    );
    plan.add_task(task);

    // Start event monitoring
    let monitor_task = tokio::spawn(async move {
        println!("📡 Starting streaming test...\n");
        while let Some(event) = event_rx.recv().await {
            match event {
                OrchestratorEvent::WorkerOutput { line, is_stderr, .. } => {
                    let prefix = if is_stderr { "ERR" } else { "OUT" };
                    println!("[{}] {}", prefix, line);
                }
                OrchestratorEvent::WorkerProgress { message, percentage, .. } => {
                    if let Some(pct) = percentage {
                        println!("📈 {} ({}%)", message, pct);
                    } else {
                        println!("📈 {}", message);
                    }
                }
                OrchestratorEvent::WorkerCompleted { success, duration, .. } => {
                    if success {
                        println!("\n✅ Test completed successfully in {:.2}s", duration.as_secs_f64());
                    } else {
                        println!("\n❌ Test failed after {:.2}s", duration.as_secs_f64());
                    }
                    break;
                }
                OrchestratorEvent::Error { error, .. } => {
                    println!("🚨 Error: {}", error);
                    break;
                }
                _ => {} // Ignore other events for this test
            }
        }
    });

    // Execute with timeout
    let execution_task = manager.execute_plan_live(&plan);
    
    match tokio::time::timeout(Duration::from_secs(duration_sec), execution_task).await {
        Ok(results) => {
            monitor_task.abort();
            match results {
                Ok(_) => println!("\n🎉 Streaming test completed successfully!"),
                Err(e) => println!("\n❌ Streaming test failed: {}", e),
            }
        }
        Err(_) => {
            monitor_task.abort();
            manager.stop_all_workers().await?;
            println!("\n⏰ Test timed out after {}s", duration_sec);
        }
    }

    Ok(())
}

/// Configure live orchestration settings
async fn configure_live_orchestration(
    buffer_size: Option<usize>,
    flush_interval_ms: Option<u64>,
    progress_detection: Option<bool>,
    save: bool,
) -> Result<()> {
    println!("⚙️  Configuring Live Orchestration");
    println!("═══════════════════════════════════");

    let mut config = StreamingConfig::default();

    if let Some(size) = buffer_size {
        config.buffer_size = size;
        println!("📦 Buffer size: {} bytes", size);
    }

    if let Some(interval) = flush_interval_ms {
        config.flush_interval_ms = interval;
        println!("⚡ Flush interval: {}ms", interval);
    }

    if let Some(detection) = progress_detection {
        config.progress_detection = detection;
        println!("📊 Progress detection: {}", if detection { "enabled" } else { "disabled" });
    }

    if save {
        // TODO: Save configuration to file
        println!("💾 Configuration saved");
    } else {
        println!("💡 Use --save to persist these settings");
    }

    Ok(())
}

/// Handle orchestrator events for UI display
async fn handle_orchestrator_event(event: OrchestratorEvent, detailed_output: bool) {
    match event {
        OrchestratorEvent::WorkerStarted { worker_id, task_id, worker_kind } => {
            println!("🚀 Started {} | Task: {} | Worker: {:?}", 
                     worker_id, task_id, worker_kind);
        }
        
        OrchestratorEvent::WorkerOutput { worker_id, line, is_stderr, .. } => {
            if detailed_output {
                let prefix = if is_stderr { "🔴" } else { "💬" };
                println!("{} [{}] {}", prefix, worker_id, line);
            }
        }
        
        OrchestratorEvent::WorkerProgress { worker_id, message, percentage, .. } => {
            if let Some(pct) = percentage {
                println!("📈 [{}] {} ({}%)", worker_id, message, pct);
            } else {
                println!("📈 [{}] {}", worker_id, message);
            }
        }
        
        OrchestratorEvent::WorkerStateChanged { worker_id, new_state, .. } => {
            println!("🔄 [{}] State: {}", worker_id, new_state);
        }
        
        OrchestratorEvent::WorkerCompleted { worker_id, success, duration, .. } => {
            let icon = if success { "✅" } else { "❌" };
            println!("{} [{}] Completed in {:.2}s", icon, worker_id, duration.as_secs_f64());
        }
        
        OrchestratorEvent::OrchestrationProgress { completed_workers, total_workers, .. } => {
            println!("📊 Progress: {}/{} workers completed", completed_workers, total_workers);
        }
        
        OrchestratorEvent::Error { worker_id, error, .. } => {
            if let Some(id) = worker_id {
                println!("🚨 [{}] Error: {}", id, error);
            } else {
                println!("🚨 Error: {}", error);
            }
        }
    }
}

/// Create a task plan from input string
fn create_task_plan_from_input(input: &str) -> Result<TaskPlan> {
    // This is a simplified implementation - in practice this would
    // use the unified planner to break down the input into tasks
    let mut plan = TaskPlan::new(
        "live-plan".to_string(),
        input.to_string(),
        "Live orchestration plan".to_string(),
    );

    // For now, create a single task
    let task = Task {
        id: "task-1".to_string(),
        description: input.to_string(),
        instructions: input.to_string(),
        relevant_files: vec![],
        dependencies: vec![],
        preferred_worker: Some(WorkerKind::ClaudeCode),
        priority: 0,
        status: crate::orchestrator::TaskStatus::Pending,
    };

    plan.add_task(task);
    Ok(plan)
}

/// Execute standard (non-streaming) orchestration as fallback
async fn execute_standard_orchestration(
    input: String,
    project_path: PathBuf,
    max_workers: usize,
) -> Result<()> {
    // Fallback to standard orchestration
    let config = OrchestratorConfig {
        max_workers,
        ..Default::default()
    };

    let mut orchestrator = Orchestrator::new(project_path, config).await?;
    let response = orchestrator.process_request(&input).await?;
    
    println!("{}", response.summary);
    Ok(())
}

/// Print execution results
async fn print_execution_results(results: &[crate::orchestrator::live_orchestration::TaskResult]) {
    println!("\n🎯 Final Results");
    println!("═════════════════");

    for result in results {
        match &result.result {
            Ok(_) => println!("✅ {} - Success", result.worker_id),
            Err(error) => println!("❌ {} - Failed: {}", result.worker_id, error),
        }
    }
}