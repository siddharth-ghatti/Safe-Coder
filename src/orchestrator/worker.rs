//! Worker module for executing tasks via external CLI agents

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::orchestrator::Task;

/// Types of CLI workers available
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WorkerKind {
    /// Claude Code CLI (https://docs.anthropic.com/en/docs/claude-code)
    ClaudeCode,
    /// Gemini CLI (https://github.com/google/gemini-cli)
    GeminiCli,
    /// Safe-Coder itself (this application)
    SafeCoder,
    /// GitHub Copilot CLI (gh copilot)
    GitHubCopilot,
}

impl Default for WorkerKind {
    fn default() -> Self {
        WorkerKind::ClaudeCode
    }
}

/// Status of a worker
#[derive(Debug, Clone)]
pub struct WorkerStatus {
    /// Task being executed
    pub task_id: String,
    /// Worker type
    pub kind: WorkerKind,
    /// Current state
    pub state: WorkerState,
    /// Output collected so far
    pub output: String,
    /// Workspace path
    pub workspace: PathBuf,
}

/// State of a worker
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerState {
    /// Worker is initializing
    Initializing,
    /// Worker is running
    Running,
    /// Worker completed successfully
    Completed,
    /// Worker failed
    Failed(String),
    /// Worker was cancelled
    Cancelled,
}

/// A worker that executes a task using an external CLI
pub struct Worker {
    /// The task to execute
    task: Task,
    /// Working directory (isolated workspace)
    workspace: PathBuf,
    /// Type of CLI to use
    kind: WorkerKind,
    /// Path to the CLI executable
    cli_path: String,
    /// Current state
    state: WorkerState,
    /// Collected output
    output: String,
    /// Child process handle (if running)
    process_handle: Option<tokio::process::Child>,
}

impl Worker {
    /// Create a new worker
    pub fn new(task: Task, workspace: PathBuf, kind: WorkerKind, cli_path: String) -> Result<Self> {
        Ok(Self {
            task,
            workspace,
            kind,
            cli_path,
            state: WorkerState::Initializing,
            output: String::new(),
            process_handle: None,
        })
    }

    /// Execute the task
    pub async fn execute(&mut self) -> Result<String, String> {
        self.state = WorkerState::Running;

        // Build the command based on worker kind
        let result = match &self.kind {
            WorkerKind::ClaudeCode => self.execute_claude_code().await,
            WorkerKind::GeminiCli => self.execute_gemini_cli().await,
            WorkerKind::SafeCoder => self.execute_safe_coder().await,
            WorkerKind::GitHubCopilot => self.execute_github_copilot().await,
        };

        match result {
            Ok(output) => {
                self.output = output.clone();
                self.state = WorkerState::Completed;
                Ok(output)
            }
            Err(e) => {
                let error_msg = e.to_string();
                self.state = WorkerState::Failed(error_msg.clone());
                Err(error_msg)
            }
        }
    }

    /// Execute using Claude Code CLI
    async fn execute_claude_code(&mut self) -> Result<String> {
        // Check if claude CLI is available
        let cli_available = Command::new(&self.cli_path)
            .arg("--version")
            .output()
            .await
            .is_ok();

        if !cli_available {
            return Err(anyhow::anyhow!(
                "Claude Code CLI not found at '{}'. Install it from https://docs.anthropic.com/en/docs/claude-code",
                self.cli_path
            ));
        }

        // Claude Code CLI usage: claude -p "prompt" for non-interactive mode
        // The -p flag enables print mode which outputs to stdout and exits
        let mut cmd = Command::new(&self.cli_path);
        cmd.current_dir(&self.workspace)
            .arg("-p")
            .arg(&self.task.instructions)
            .arg("--dangerously-skip-permissions") // Skip permission prompts for automated use
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.run_command(cmd).await
    }

    /// Execute using Gemini CLI
    async fn execute_gemini_cli(&mut self) -> Result<String> {
        // Check if gemini CLI is available
        let cli_available = Command::new(&self.cli_path)
            .arg("--version")
            .output()
            .await
            .is_ok();

        if !cli_available {
            return Err(anyhow::anyhow!(
                "Gemini CLI not found at '{}'. Install it from https://github.com/google/gemini-cli",
                self.cli_path
            ));
        }

        // Gemini CLI usage varies - adjust as needed
        let mut cmd = Command::new(&self.cli_path);
        cmd.current_dir(&self.workspace)
            .arg("--prompt")
            .arg(&self.task.instructions)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.run_command(cmd).await
    }

    /// Execute using Safe-Coder itself (recursive orchestration)
    async fn execute_safe_coder(&mut self) -> Result<String> {
        // Safe-Coder can be invoked via its own binary
        // First try the current executable, then fall back to cli_path
        let exe_path = std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| self.cli_path.clone());

        // Check if safe-coder CLI is available
        let cli_available = Command::new(&exe_path)
            .arg("--version")
            .output()
            .await
            .is_ok();

        if !cli_available {
            return Err(anyhow::anyhow!(
                "Safe-Coder CLI not found at '{}'. Make sure the binary is in your PATH.",
                exe_path
            ));
        }

        // Safe-Coder usage: safe-coder act "prompt" for non-interactive mode
        let mut cmd = Command::new(&exe_path);
        cmd.current_dir(&self.workspace)
            .arg("act")
            .arg(&self.task.instructions)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.run_command(cmd).await
    }

    /// Execute using GitHub Copilot CLI (gh copilot)
    async fn execute_github_copilot(&mut self) -> Result<String> {
        // GitHub Copilot CLI is accessed via `gh copilot` command
        // Check if gh CLI is available with copilot extension
        let cli_available = Command::new("gh")
            .args(["copilot", "--help"])
            .output()
            .await
            .is_ok();

        if !cli_available {
            return Err(anyhow::anyhow!(
                "GitHub Copilot CLI not found. Install it with:\n\
                 1. Install GitHub CLI: https://cli.github.com/\n\
                 2. Install Copilot extension: gh extension install github/gh-copilot\n\
                 3. Authenticate: gh auth login"
            ));
        }

        // GitHub Copilot CLI usage: gh copilot suggest -t shell "prompt"
        // For code suggestions, we use the suggest command
        let mut cmd = Command::new("gh");
        cmd.current_dir(&self.workspace)
            .arg("copilot")
            .arg("suggest")
            .arg("-t")
            .arg("shell") // Can be "shell", "git", or "gh"
            .arg(&self.task.instructions)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.run_command(cmd).await
    }

    /// Run a command and collect output with timeout
    async fn run_command(&mut self, mut cmd: Command) -> Result<String> {
        let mut child = cmd.spawn().context("Failed to spawn CLI process")?;

        // Store the child handle for potential cancellation
        let child_id = child.id();

        // Capture stdout
        let stdout = child.stdout.take().context("Failed to capture stdout")?;

        let stderr = child.stderr.take().context("Failed to capture stderr")?;

        // Read stdout and stderr concurrently to avoid deadlock
        let stdout_reader = BufReader::new(stdout);
        let stderr_reader = BufReader::new(stderr);

        // Spawn tasks to read both streams concurrently
        let stdout_task = tokio::spawn(async move {
            let mut output = String::new();
            let mut lines = stdout_reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                output.push_str(&line);
                output.push('\n');
            }
            output
        });

        let stderr_task = tokio::spawn(async move {
            let mut errors = String::new();
            let mut lines = stderr_reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                errors.push_str(&line);
                errors.push('\n');
            }
            errors
        });

        // Wait for process with a timeout (5 minutes max)
        let timeout_duration = tokio::time::Duration::from_secs(300);

        let wait_result = tokio::time::timeout(timeout_duration, async {
            // Wait for both streams to complete
            let (stdout_result, stderr_result) = tokio::join!(stdout_task, stderr_task);
            let output = stdout_result.unwrap_or_default();
            let errors = stderr_result.unwrap_or_default();

            // Wait for process to complete
            let status = child.wait().await.context("Failed to wait for process")?;

            Ok::<(String, String, std::process::ExitStatus), anyhow::Error>((
                output, errors, status,
            ))
        })
        .await;

        match wait_result {
            Ok(Ok((output, errors, status))) => {
                if status.success() {
                    Ok(output)
                } else {
                    Err(anyhow::anyhow!(
                        "CLI process exited with status {}: {}",
                        status.code().unwrap_or(-1),
                        errors
                    ))
                }
            }
            Ok(Err(e)) => Err(e),
            Err(_) => {
                // Timeout - try to kill the process
                if let Some(pid) = child_id {
                    let _ = tokio::process::Command::new("kill")
                        .arg("-9")
                        .arg(pid.to_string())
                        .output()
                        .await;
                }
                Err(anyhow::anyhow!(
                    "CLI process timed out after {} seconds",
                    timeout_duration.as_secs()
                ))
            }
        }
    }

    /// Get current status
    pub fn status(&self) -> WorkerStatus {
        WorkerStatus {
            task_id: self.task.id.clone(),
            kind: self.kind.clone(),
            state: self.state.clone(),
            output: self.output.clone(),
            workspace: self.workspace.clone(),
        }
    }

    /// Cancel the worker
    pub async fn cancel(&mut self) -> Result<()> {
        if let Some(mut process) = self.process_handle.take() {
            process.kill().await.context("Failed to kill process")?;
        }
        self.state = WorkerState::Cancelled;
        Ok(())
    }

    /// Check if worker is still running
    pub fn is_running(&self) -> bool {
        matches!(self.state, WorkerState::Running)
    }

    /// Get the task being executed
    pub fn task(&self) -> &Task {
        &self.task
    }

    /// Get collected output
    pub fn output(&self) -> &str {
        &self.output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::TaskStatus;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_worker_creation() {
        let task = Task {
            id: "test-1".to_string(),
            description: "Test task".to_string(),
            instructions: "Do something".to_string(),
            relevant_files: vec![],
            dependencies: vec![],
            preferred_worker: None,
            priority: 0,
            status: TaskStatus::Pending,
        };

        let workspace = tempdir().unwrap();
        let worker = Worker::new(
            task,
            workspace.path().to_path_buf(),
            WorkerKind::ClaudeCode,
            "claude".to_string(),
        )
        .unwrap();

        assert_eq!(worker.kind, WorkerKind::ClaudeCode);
        assert!(matches!(worker.state, WorkerState::Initializing));
    }
}
