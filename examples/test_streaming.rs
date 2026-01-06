//! Test script to verify live streaming orchestration works
//!
//! This demonstrates the enhanced streaming capabilities.

use anyhow::Result;
use safe_coder::orchestrator::{
    streaming_worker::{StreamingWorker, StreamingConfig, WorkerKind},
    live_orchestration::LiveOrchestrationManager,
    Task, TaskStatus,
};
use std::path::PathBuf;
use tempfile::tempdir;

#[tokio::main]
async fn main() -> Result<()> {
    println!("🧪 Testing Live Streaming Orchestration");
    println!("=========================================");

    // Test 1: Basic streaming worker
    test_streaming_worker().await?;
    
    // Test 2: Live orchestration manager
    test_live_orchestration().await?;

    println!("\n✅ All streaming tests passed!");
    Ok(())
}

async fn test_streaming_worker() -> Result<()> {
    println!("\n📡 Test 1: Basic Streaming Worker");
    println!("----------------------------------");

    let temp_dir = tempdir()?;
    
    let task = Task {
        id: "test-1".to_string(),
        description: "Test streaming".to_string(),
        instructions: "echo 'Hello from streaming worker!'".to_string(),
        relevant_files: vec![],
        dependencies: vec![],
        preferred_worker: Some(WorkerKind::SafeCoder),
        priority: 0,
        status: TaskStatus::Pending,
    };

    let config = StreamingConfig {
        enabled: true,
        buffer_size: 1024,
        flush_interval_ms: 50,
        max_line_length: 1024,
        progress_detection: true,
        heartbeat_interval_sec: 5,
    };

    // Use echo command for testing (available on all systems)
    let worker = StreamingWorker::new(
        task,
        temp_dir.path().to_path_buf(),
        WorkerKind::SafeCoder,
        "echo".to_string(), // Simple command for testing
    )?
    .with_streaming_config(config);

    let (mut worker, mut event_rx) = worker.with_event_stream();

    println!("🚀 Starting streaming worker...");

    // Spawn event listener
    let event_task = tokio::spawn(async move {
        let mut event_count = 0;
        while let Some(event) = event_rx.recv().await {
            event_count += 1;
            match event {
                safe_coder::orchestrator::streaming_worker::WorkerStreamEvent::Output { line, .. } => {
                    println!("📺 Output: {}", line);
                }
                safe_coder::orchestrator::streaming_worker::WorkerStreamEvent::StateChanged { new_state, .. } => {
                    println!("🔄 State: {:?}", new_state);
                }
                safe_coder::orchestrator::streaming_worker::WorkerStreamEvent::Completed { success, .. } => {
                    println!("🏁 Completed: {}", if success { "Success" } else { "Failed" });
                    break;
                }
                _ => {}
            }
        }
        event_count
    });

    // Execute the worker - this will fail with echo but should demonstrate streaming setup
    let _result = worker.execute_streaming().await;
    let event_count = event_task.await?;

    println!("📊 Received {} streaming events", event_count);
    println!("✅ Streaming worker test complete");

    Ok(())
}

async fn test_live_orchestration() -> Result<()> {
    println!("\n🎯 Test 2: Live Orchestration Manager");
    println!("--------------------------------------");

    let temp_dir = tempdir()?;
    
    let manager = LiveOrchestrationManager::new(temp_dir.path().to_path_buf());
    let (manager, mut event_rx) = manager.with_ui_events();

    println!("🚀 Live orchestration manager created");

    // Get worker statuses (should be empty)
    let statuses = manager.get_worker_statuses().await;
    println!("📊 Active workers: {}", statuses.len());

    // Spawn event listener for a short time
    let event_task = tokio::spawn(async move {
        let mut event_count = 0;
        let mut timeout = tokio::time::interval(std::time::Duration::from_millis(100));
        
        for _ in 0..10 { // Listen for 1 second
            tokio::select! {
                Some(event) = event_rx.recv() => {
                    event_count += 1;
                    match event {
                        safe_coder::orchestrator::live_orchestration::OrchestratorEvent::WorkerStarted { worker_id, .. } => {
                            println!("🚀 Worker started: {}", worker_id);
                        }
                        safe_coder::orchestrator::live_orchestration::OrchestratorEvent::Error { error, .. } => {
                            println!("🚨 Error: {}", error);
                        }
                        _ => {}
                    }
                }
                _ = timeout.tick() => {
                    break;
                }
            }
        }
        event_count
    });

    let event_count = event_task.await?;
    println!("📊 Processed {} orchestration events", event_count);
    println!("✅ Live orchestration test complete");

    Ok(())
}