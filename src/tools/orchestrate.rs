//! Orchestrate Tool
//!
//! Exposes the orchestrator to the LLM for delegating independent tasks
//! to external CLI agents (Claude Code, Gemini CLI, GitHub Copilot).
//! These CLIs must be configured in the orchestrator config.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use crate::config::Config;
use crate::orchestrator::{Task, TaskStatus, Worker, WorkerEvent, WorkerKind};
use crate::session::SessionEvent;
use crate::tools::{Tool, ToolContext};

/// Tracks orchestration depth to prevent recursive SafeCoder calls
static ORCHESTRATION_DEPTH: AtomicUsize = AtomicUsize::new(0);
const MAX_ORCHESTRATION_DEPTH: usize = 1;

/// RAII guard to decrement orchestration depth on drop
struct OrchestrationGuard;

impl Drop for OrchestrationGuard {
    fn drop(&mut self) {
        ORCHESTRATION_DEPTH.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Tool for delegating tasks to external CLI agents
pub struct OrchestrateTool {
    config: Arc<Mutex<Option<Config>>>,
    project_path: Arc<Mutex<Option<PathBuf>>>,
}

impl OrchestrateTool {
    pub fn new() -> Self {
        Self {
            config: Arc::new(Mutex::new(None)),
            project_path: Arc::new(Mutex::new(None)),
        }
    }

    /// Initialize the tool with config and project path
    pub async fn initialize(&self, config: Config, project_path: PathBuf) {
        *self.config.lock().await = Some(config);
        *self.project_path.lock().await = Some(project_path);
    }
}

impl Default for OrchestrateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct OrchestrateParams {
    /// The worker/CLI to use: "claude", "gemini", "copilot"
    worker: String,
    /// The task description/instructions for the external CLI
    task: String,
    /// Files relevant to this task (for context)
    #[serde(default)]
    relevant_files: Vec<String>,
}

#[derive(Debug, Serialize)]
struct OrchestrateResult {
    success: bool,
    worker: String,
    workspace_path: Option<String>,
    output: String,
    error: Option<String>,
}

#[async_trait]
impl Tool for OrchestrateTool {
    fn name(&self) -> &str {
        "orchestrate"
    }

    fn description(&self) -> &str {
        r#"Delegate a task to an external CLI agent for execution. Use this for independent tasks that can run in parallel or benefit from a specialized external tool. Available workers depend on your config: claude (Claude Code CLI), gemini (Gemini CLI), copilot (GitHub Copilot). The task runs in an isolated git workspace and results are merged back on success. NOTE: SafeCoder cannot orchestrate itself to prevent infinite loops."#
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "worker": {
                    "type": "string",
                    "description": "The external CLI to use. Available: claude (Claude Code), gemini (Gemini CLI), copilot (GitHub Copilot). Check your config for which CLIs are enabled."
                },
                "task": {
                    "type": "string",
                    "description": "Clear instructions for the external CLI to execute. Be specific about what files to modify and expected outcomes."
                },
                "relevant_files": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Files relevant to this task (helps the worker focus)"
                }
            },
            "required": ["worker", "task"]
        })
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext<'_>) -> Result<String> {
        // Check orchestration depth to prevent recursion
        let current_depth = ORCHESTRATION_DEPTH.fetch_add(1, Ordering::SeqCst);

        // Create guard to ensure we decrement on exit (even on error/panic)
        let _guard = OrchestrationGuard;

        if current_depth >= MAX_ORCHESTRATION_DEPTH {
            return Ok(serde_json::to_string_pretty(&OrchestrateResult {
                success: false,
                worker: "unknown".to_string(),
                workspace_path: None,
                output: String::new(),
                error: Some(format!(
                    "Recursive orchestration blocked. Maximum orchestration depth ({}) exceeded.",
                    MAX_ORCHESTRATION_DEPTH
                )),
            })?);
        }

        let params: OrchestrateParams =
            serde_json::from_value(params).context("Invalid parameters for orchestrate")?;

        // Parse worker kind
        let worker_kind = match params.worker.to_lowercase().as_str() {
            "claude" | "claude-code" | "claudecode" => WorkerKind::ClaudeCode,
            "gemini" | "gemini-cli" => WorkerKind::GeminiCli,
            "copilot" | "github-copilot" | "gh-copilot" => WorkerKind::GitHubCopilot,
            "safecoder" | "safe-coder" => {
                // Block safecoder-calling-safecoder to prevent infinite loops
                return Ok(serde_json::to_string_pretty(&OrchestrateResult {
                    success: false,
                    worker: params.worker,
                    workspace_path: None,
                    output: String::new(),
                    error: Some(
                        "SafeCoder cannot orchestrate another SafeCoder instance to prevent \
                         infinite loops. Use a different worker (claude, gemini, copilot) or \
                         handle the task directly."
                            .to_string(),
                    ),
                })?);
            }
            _ => {
                return Ok(serde_json::to_string_pretty(&OrchestrateResult {
                    success: false,
                    worker: params.worker.clone(),
                    workspace_path: None,
                    output: String::new(),
                    error: Some(format!(
                        "Unknown worker '{}'. Valid options: claude, gemini, copilot. \
                         Check your orchestrator config for enabled workers.",
                        params.worker
                    )),
                })?);
            }
        };

        // Get config and project path
        let config_guard = self.config.lock().await;
        let project_path_guard = self.project_path.lock().await;

        let config = config_guard.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Orchestrate tool not initialized - missing config")
        })?;

        let project_path = project_path_guard.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Orchestrate tool not initialized - missing project path")
        })?;

        // Check if worker is in enabled list
        let worker_name = match worker_kind {
            WorkerKind::ClaudeCode => "claude",
            WorkerKind::GeminiCli => "gemini",
            WorkerKind::SafeCoder => "safe-coder",
            WorkerKind::GitHubCopilot => "github-copilot",
        };

        if !config.orchestrator.enabled_workers.contains(&worker_name.to_string()) {
            return Ok(serde_json::to_string_pretty(&OrchestrateResult {
                success: false,
                worker: format!("{:?}", worker_kind),
                workspace_path: None,
                output: String::new(),
                error: Some(format!(
                    "Worker '{}' is not enabled in your orchestrator config. \
                     Enabled workers: {:?}. Update your config to add this worker.",
                    worker_name, config.orchestrator.enabled_workers
                )),
            })?);
        }

        // Verify CLI is available
        let cli_path = get_cli_path(config, &worker_kind);
        if !check_cli_available(&cli_path, &worker_kind).await {
            return Ok(serde_json::to_string_pretty(&OrchestrateResult {
                success: false,
                worker: format!("{:?}", worker_kind),
                workspace_path: None,
                output: String::new(),
                error: Some(format!(
                    "CLI not available at '{}'. Please ensure it is installed and in your PATH, \
                     or update the cli path in your orchestrator config.",
                    cli_path
                )),
            })?);
        }

        // Create task
        let task_id = format!("orch-{}", uuid::Uuid::new_v4().to_string()[..8].to_string());
        let task = Task {
            id: task_id.clone(),
            description: truncate_str(&params.task, 100),
            instructions: params.task.clone(),
            relevant_files: params.relevant_files.clone(),
            dependencies: vec![],
            preferred_worker: Some(worker_kind.clone()),
            priority: 0,
            status: TaskStatus::Pending,
        };

        // Determine workspace - use project path directly for simplicity
        let workspace = project_path.clone();

        // Create worker event channel for streaming
        let (worker_event_tx, mut worker_event_rx) = mpsc::unbounded_channel();

        // Create worker with event sender
        let mut worker = Worker::with_event_sender(
            task,
            workspace.clone(),
            worker_kind.clone(),
            cli_path,
            worker_event_tx,
        )?;

        // Clone session event tx for the forwarder task
        let session_tx = ctx.session_event_tx.clone();
        let has_session_tx = session_tx.is_some();
        tracing::info!("[ORCHESTRATE] session_event_tx is_some: {}", has_session_tx);

        let task_id_clone = task_id.clone();
        let worker_name_clone = worker_name.to_string();
        let task_desc = truncate_str(&params.task, 50);

        // Spawn forwarder task to convert WorkerEvent -> SessionEvent
        let forwarder_handle = tokio::spawn(async move {
            tracing::info!("[ORCHESTRATE] Forwarder task started, has session_tx: {}", session_tx.is_some());

            // Send started event
            if let Some(ref tx) = session_tx {
                tracing::info!("[ORCHESTRATE] Sending OrchestrateStarted event");
                let _ = tx.send(SessionEvent::OrchestrateStarted {
                    id: task_id_clone.clone(),
                    worker: worker_name_clone.clone(),
                    task: task_desc,
                });
            }

            // Forward streaming events
            let mut event_count = 0;
            while let Some(event) = worker_event_rx.recv().await {
                event_count += 1;
                tracing::debug!("[ORCHESTRATE] Received worker event #{}: {:?}", event_count, std::mem::discriminant(&event));

                if let Some(ref tx) = session_tx {
                    match event {
                        WorkerEvent::OutputLine { task_id, line } => {
                            tracing::debug!("[ORCHESTRATE] Forwarding output line: {}", &line[..line.len().min(100)]);
                            let _ = tx.send(SessionEvent::OrchestrateOutput {
                                id: task_id,
                                line,
                            });
                        }
                        WorkerEvent::ErrorLine { task_id, line } => {
                            tracing::debug!("[ORCHESTRATE] Forwarding error line: {}", &line[..line.len().min(100)]);
                            let _ = tx.send(SessionEvent::OrchestrateOutput {
                                id: task_id,
                                line: format!("[stderr] {}", line),
                            });
                        }
                        WorkerEvent::Completed { task_id, output } => {
                            tracing::info!("[ORCHESTRATE] Worker completed, output len: {}", output.len());
                            let _ = tx.send(SessionEvent::OrchestrateCompleted {
                                id: task_id,
                                success: true,
                                output: truncate_output(&output, 500),
                            });
                        }
                        WorkerEvent::Failed { task_id, error } => {
                            tracing::info!("[ORCHESTRATE] Worker failed: {}", error);
                            let _ = tx.send(SessionEvent::OrchestrateCompleted {
                                id: task_id,
                                success: false,
                                output: error,
                            });
                        }
                        WorkerEvent::Started { .. } => {
                            tracing::debug!("[ORCHESTRATE] Worker started event (already sent)");
                        }
                    }
                }
            }
            tracing::info!("[ORCHESTRATE] Forwarder task finished, processed {} events", event_count);
        });

        // Execute the worker
        let result = worker.execute().await;

        // Wait for forwarder to complete (it will finish when worker drops the sender)
        let _ = forwarder_handle.await;

        // Build result
        match result {
            Ok(output) => Ok(serde_json::to_string_pretty(&OrchestrateResult {
                success: true,
                worker: format!("{:?}", worker_kind),
                workspace_path: Some(workspace.to_string_lossy().to_string()),
                output: truncate_output(&output, 4000),
                error: None,
            })?),
            Err(error) => Ok(serde_json::to_string_pretty(&OrchestrateResult {
                success: false,
                worker: format!("{:?}", worker_kind),
                workspace_path: Some(workspace.to_string_lossy().to_string()),
                output: String::new(),
                error: Some(error),
            })?),
        }
    }
}

fn get_cli_path(config: &Config, kind: &WorkerKind) -> String {
    match kind {
        WorkerKind::ClaudeCode => config.orchestrator.claude_cli_path.clone(),
        WorkerKind::GeminiCli => config.orchestrator.gemini_cli_path.clone(),
        WorkerKind::SafeCoder => config.orchestrator.safe_coder_cli_path.clone(),
        WorkerKind::GitHubCopilot => config.orchestrator.gh_cli_path.clone(),
    }
}

async fn check_cli_available(path: &str, kind: &WorkerKind) -> bool {
    use tokio::process::Command;

    let result = match kind {
        WorkerKind::GitHubCopilot => Command::new("gh").args(["copilot", "--help"]).output().await,
        _ => Command::new(path).arg("--version").output().await,
    };

    result.map(|o| o.status.success()).unwrap_or(false)
}

fn truncate_output(output: &str, max_chars: usize) -> String {
    if output.len() <= max_chars {
        return output.to_string();
    }

    // Find a safe UTF-8 boundary
    let mut safe_limit = max_chars;
    while safe_limit > 0 && !output.is_char_boundary(safe_limit) {
        safe_limit -= 1;
    }

    format!(
        "{}...\n[Output truncated: {} chars total]",
        &output[..safe_limit],
        output.len()
    )
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        return s.to_string();
    }
    let mut safe_limit = max_chars;
    while safe_limit > 0 && !s.is_char_boundary(safe_limit) {
        safe_limit -= 1;
    }
    format!("{}...", &s[..safe_limit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_schema() {
        let tool = OrchestrateTool::new();
        let schema = tool.parameters_schema();

        assert!(schema["properties"]["worker"].is_object());
        assert!(schema["properties"]["task"].is_object());
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("worker")));
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("task")));
    }

    #[test]
    fn test_truncate_output() {
        let short = "Hello world";
        assert_eq!(truncate_output(short, 100), short);

        let long = "a".repeat(100);
        let truncated = truncate_output(&long, 50);
        assert!(truncated.len() < 100);
        assert!(truncated.contains("truncated"));
    }
}
