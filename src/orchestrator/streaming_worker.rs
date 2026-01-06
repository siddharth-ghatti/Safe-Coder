//! Enhanced worker with live streaming support
//!
//! This module provides real-time streaming from CLI workers with proper
//! buffering control and live progress updates.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant};

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
    /// Real-time streaming enabled
    pub streaming_enabled: bool,
    /// Lines processed so far
    pub lines_processed: usize,
    /// Execution start time
    pub started_at: Option<Instant>,
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

/// Real-time streaming events from workers
#[derive(Debug, Clone)]
pub enum WorkerStreamEvent {
    /// New output line received
    Output {
        line: String,
        timestamp: Instant,
        is_stderr: bool,
    },
    /// Worker state changed
    StateChanged {
        old_state: WorkerState,
        new_state: WorkerState,
        timestamp: Instant,
    },
    /// Progress indicator (for long-running operations)
    Progress {
        message: String,
        percentage: Option<f32>,
        timestamp: Instant,
    },
    /// Worker completed
    Completed {
        success: bool,
        final_output: String,
        duration: Duration,
        timestamp: Instant,
    },
}

/// Configuration for worker streaming
#[derive(Debug, Clone)]
pub struct StreamingConfig {
    /// Enable real-time streaming
    pub enabled: bool,
    /// Buffer size for reading output (bytes)
    pub buffer_size: usize,
    /// Flush interval for live updates (milliseconds)
    pub flush_interval_ms: u64,
    /// Maximum line length before truncation
    pub max_line_length: usize,
    /// Enable progress detection for long operations
    pub progress_detection: bool,
    /// Heartbeat interval to show worker is alive (seconds)
    pub heartbeat_interval_sec: u64,
}

impl Default for StreamingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            buffer_size: 8192, // 8KB buffer
            flush_interval_ms: 100, // 100ms flush interval
            max_line_length: 4096, // 4KB max line
            progress_detection: true,
            heartbeat_interval_sec: 10, // 10 second heartbeat
        }
    }
}

/// A worker that executes a task using an external CLI with live streaming
pub struct StreamingWorker {
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
    /// Streaming configuration
    streaming_config: StreamingConfig,
    /// Event sender for real-time updates
    event_sender: Option<mpsc::UnboundedSender<WorkerStreamEvent>>,
    /// Execution start time
    started_at: Option<Instant>,
    /// Lines processed counter
    lines_processed: usize,
}

impl StreamingWorker {
    /// Create a new streaming worker
    pub fn new(
        task: Task,
        workspace: PathBuf,
        kind: WorkerKind,
        cli_path: String,
    ) -> Result<Self> {
        Ok(Self {
            task,
            workspace,
            kind,
            cli_path,
            state: WorkerState::Initializing,
            output: String::new(),
            process_handle: None,
            streaming_config: StreamingConfig::default(),
            event_sender: None,
            started_at: None,
            lines_processed: 0,
        })
    }

    /// Configure streaming options
    pub fn with_streaming_config(mut self, config: StreamingConfig) -> Self {
        self.streaming_config = config;
        self
    }

    /// Set up event streaming channel
    pub fn with_event_stream(mut self) -> (Self, mpsc::UnboundedReceiver<WorkerStreamEvent>) {
        let (sender, receiver) = mpsc::unbounded_channel();
        self.event_sender = Some(sender);
        (self, receiver)
    }

    /// Execute the task with live streaming
    pub async fn execute_streaming(&mut self) -> Result<String, String> {
        self.started_at = Some(Instant::now());
        self.set_state(WorkerState::Running).await;

        // Build the command based on worker kind
        let result = match &self.kind {
            WorkerKind::ClaudeCode => self.execute_claude_code_streaming().await,
            WorkerKind::GeminiCli => self.execute_gemini_cli_streaming().await,
            WorkerKind::SafeCoder => self.execute_safe_coder_streaming().await,
            WorkerKind::GitHubCopilot => self.execute_github_copilot_streaming().await,
        };

        match result {
            Ok(output) => {
                self.output = output.clone();
                self.set_state(WorkerState::Completed).await;
                self.emit_completion(true, &output).await;
                Ok(output)
            }
            Err(e) => {
                let error_msg = e.to_string();
                self.set_state(WorkerState::Failed(error_msg.clone())).await;
                self.emit_completion(false, &error_msg).await;
                Err(error_msg)
            }
        }
    }

    /// Execute using Claude Code CLI with optimized streaming
    async fn execute_claude_code_streaming(&mut self) -> Result<String> {
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

        // Claude Code CLI with optimized flags for streaming
        let mut cmd = Command::new(&self.cli_path);
        cmd.current_dir(&self.workspace)
            .arg("-p") // Print mode for non-interactive use
            .arg(&self.task.instructions)
            .arg("--dangerously-skip-permissions") // Skip permission prompts
            .arg("--no-color") // Disable color codes that can break parsing
            .arg("--streaming") // Enable streaming if supported
            .env("FORCE_COLOR", "0") // Ensure no color codes
            .env("NO_COLOR", "1") // Another way to disable colors
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.run_command_streaming(cmd).await
    }

    /// Execute using Gemini CLI with streaming
    async fn execute_gemini_cli_streaming(&mut self) -> Result<String> {
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

        let mut cmd = Command::new(&self.cli_path);
        cmd.current_dir(&self.workspace)
            .arg("--prompt")
            .arg(&self.task.instructions)
            .arg("--stream") // Enable streaming if supported
            .arg("--no-color") // Disable colors
            .env("FORCE_COLOR", "0")
            .env("NO_COLOR", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.run_command_streaming(cmd).await
    }

    /// Execute using Safe-Coder itself with streaming
    async fn execute_safe_coder_streaming(&mut self) -> Result<String> {
        let exe_path = std::env::current_exe()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| self.cli_path.clone());

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

        let mut cmd = Command::new(&exe_path);
        cmd.current_dir(&self.workspace)
            .arg("act")
            .arg(&self.task.instructions)
            .arg("--streaming") // Enable streaming mode
            .arg("--no-tui") // Disable TUI for clean output
            .arg("--no-color") // Disable colors
            .env("FORCE_COLOR", "0")
            .env("NO_COLOR", "1")
            .env("SAFE_CODER_STREAMING", "1") // Signal streaming mode
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.run_command_streaming(cmd).await
    }

    /// Execute using GitHub Copilot CLI with streaming
    async fn execute_github_copilot_streaming(&mut self) -> Result<String> {
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

        let mut cmd = Command::new("gh");
        cmd.current_dir(&self.workspace)
            .arg("copilot")
            .arg("suggest")
            .arg("-t")
            .arg("shell")
            .arg(&self.task.instructions)
            .env("FORCE_COLOR", "0")
            .env("NO_COLOR", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.run_command_streaming(cmd).await
    }

    /// Run a command with live streaming and real-time updates
    async fn run_command_streaming(&mut self, mut cmd: Command) -> Result<String> {
        let mut child = cmd.spawn().context("Failed to spawn CLI process")?;
        let child_id = child.id();
        self.process_handle = Some(child);

        // Take stdout and stderr for streaming
        let stdout = child.stdout.take().context("Failed to capture stdout")?;
        let stderr = child.stderr.take().context("Failed to capture stderr")?;

        let mut stdout_reader = BufReader::with_capacity(self.streaming_config.buffer_size, stdout);
        let mut stderr_reader = BufReader::with_capacity(self.streaming_config.buffer_size, stderr);

        let mut combined_output = String::new();
        let flush_interval = Duration::from_millis(self.streaming_config.flush_interval_ms);
        let heartbeat_interval = Duration::from_secs(self.streaming_config.heartbeat_interval_sec);
        let mut last_heartbeat = Instant::now();
        let mut last_flush = Instant::now();

        // Buffers for partial lines
        let mut stdout_buffer = Vec::new();
        let mut stderr_buffer = Vec::new();

        loop {
            let mut any_activity = false;
            let now = Instant::now();

            // Read from stdout (non-blocking)
            if self.read_stream_chunk(&mut stdout_reader, &mut stdout_buffer, false, &mut combined_output).await? {
                any_activity = true;
            }

            // Read from stderr (non-blocking)
            if self.read_stream_chunk(&mut stderr_reader, &mut stderr_buffer, true, &mut combined_output).await? {
                any_activity = true;
            }

            // Check if process is still running
            if let Some(ref mut process) = self.process_handle {
                match process.try_wait() {
                    Ok(Some(status)) => {
                        // Process finished - read any remaining output
                        self.flush_remaining_output(&mut stdout_reader, &mut stdout_buffer, false, &mut combined_output).await?;
                        self.flush_remaining_output(&mut stderr_reader, &mut stderr_buffer, true, &mut combined_output).await?;

                        if status.success() {
                            return Ok(combined_output);
                        } else {
                            return Err(anyhow::anyhow!(
                                "CLI process exited with status {}: Last output: {}",
                                status.code().unwrap_or(-1),
                                combined_output.lines().rev().take(5).collect::<Vec<_>>().join("\n")
                            ));
                        }
                    }
                    Ok(None) => {
                        // Process still running
                    }
                    Err(e) => {
                        return Err(anyhow::anyhow!("Error waiting for process: {}", e));
                    }
                }
            }

            // Send heartbeat if no activity
            if now.duration_since(last_heartbeat) >= heartbeat_interval {
                self.emit_progress("Worker is active...".to_string(), None).await;
                last_heartbeat = now;
            }

            // Force flush periodically even without newlines
            if now.duration_since(last_flush) >= flush_interval {
                self.flush_partial_lines(&mut stdout_buffer, false, &mut combined_output).await;
                self.flush_partial_lines(&mut stderr_buffer, true, &mut combined_output).await;
                last_flush = now;
            }

            // If no activity, sleep briefly to avoid busy waiting
            if !any_activity {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }

            // Timeout check (5 minutes default)
            if now.duration_since(self.started_at.unwrap_or(now)) > Duration::from_secs(300) {
                if let Some(pid) = child_id {
                    let _ = Command::new("kill")
                        .arg("-9")
                        .arg(pid.to_string())
                        .output()
                        .await;
                }
                return Err(anyhow::anyhow!("CLI process timed out after 5 minutes"));
            }
        }
    }

    /// Read a chunk from a stream without blocking
    async fn read_stream_chunk(
        &mut self,
        reader: &mut BufReader<impl tokio::io::AsyncRead + Unpin>,
        buffer: &mut Vec<u8>,
        is_stderr: bool,
        combined_output: &mut String,
    ) -> Result<bool> {
        let mut temp_buffer = vec![0u8; 1024];
        
        // Try to read without blocking
        match tokio::time::timeout(Duration::from_millis(1), reader.read(&mut temp_buffer)).await {
            Ok(Ok(0)) => return Ok(false), // EOF
            Ok(Ok(n)) => {
                buffer.extend_from_slice(&temp_buffer[..n]);
                self.process_buffer_lines(buffer, is_stderr, combined_output).await;
                Ok(true)
            }
            Ok(Err(_)) | Err(_) => Ok(false), // No data available or error
        }
    }

    /// Process complete lines from buffer and emit them
    async fn process_buffer_lines(
        &mut self,
        buffer: &mut Vec<u8>,
        is_stderr: bool,
        combined_output: &mut String,
    ) {
        let mut start = 0;
        while let Some(newline_pos) = buffer[start..].iter().position(|&b| b == b'\n') {
            let line_end = start + newline_pos;
            if let Ok(line) = std::str::from_utf8(&buffer[start..line_end]) {
                let line = line.trim_end_matches('\r'); // Handle Windows line endings
                
                // Truncate very long lines to prevent memory issues
                let line = if line.len() > self.streaming_config.max_line_length {
                    &line[..self.streaming_config.max_line_length]
                } else {
                    line
                };

                self.emit_output_line(line.to_string(), is_stderr).await;
                combined_output.push_str(line);
                combined_output.push('\n');
                self.lines_processed += 1;

                // Detect progress indicators
                if self.streaming_config.progress_detection {
                    self.detect_progress(line).await;
                }
            }
            start = line_end + 1;
        }

        // Keep any remaining partial line in buffer
        if start > 0 {
            buffer.drain(..start);
        }
    }

    /// Flush any remaining partial lines when process completes
    async fn flush_remaining_output(
        &mut self,
        reader: &mut BufReader<tokio::process::ChildStdout>,
        buffer: &mut Vec<u8>,
        is_stderr: bool,
        combined_output: &mut String,
    ) -> Result<()> {
        // Read any remaining data
        let mut remaining = Vec::new();
        reader.read_to_end(&mut remaining).await?;
        buffer.extend(remaining);

        // Process remaining complete lines
        self.process_buffer_lines(buffer, is_stderr, combined_output).await;

        // Flush any final partial line
        self.flush_partial_lines(buffer, is_stderr, combined_output).await;

        Ok(())
    }

    /// Flush partial lines (lines without newlines)
    async fn flush_partial_lines(
        &mut self,
        buffer: &mut Vec<u8>,
        is_stderr: bool,
        combined_output: &mut String,
    ) {
        if !buffer.is_empty() {
            if let Ok(line) = std::str::from_utf8(buffer) {
                let line = line.trim();
                if !line.is_empty() {
                    self.emit_output_line(format!("{} [partial]", line), is_stderr).await;
                    combined_output.push_str(line);
                    self.lines_processed += 1;
                }
            }
            buffer.clear();
        }
    }

    /// Detect progress indicators in output
    async fn detect_progress(&mut self, line: &str) {
        let line_lower = line.to_lowercase();
        
        // Common progress indicators
        if line_lower.contains("progress:") || line_lower.contains("processing") {
            self.emit_progress(line.to_string(), None).await;
        } else if line_lower.contains("%") {
            // Try to extract percentage
            if let Some(percentage) = self.extract_percentage(line) {
                self.emit_progress(line.to_string(), Some(percentage)).await;
            }
        } else if line_lower.contains("building") || line_lower.contains("compiling") {
            self.emit_progress(format!("Build: {}", line), None).await;
        } else if line_lower.contains("testing") || line_lower.contains("running tests") {
            self.emit_progress(format!("Testing: {}", line), None).await;
        }
    }

    /// Extract percentage from a line
    fn extract_percentage(&self, line: &str) -> Option<f32> {
        use regex::Regex;
        let re = Regex::new(r"(\d+(?:\.\d+)?)\s*%").ok()?;
        
        if let Some(caps) = re.captures(line) {
            caps.get(1)?.as_str().parse().ok()
        } else {
            None
        }
    }

    /// Emit a real-time output line
    async fn emit_output_line(&self, line: String, is_stderr: bool) {
        if let Some(ref sender) = self.event_sender {
            let _ = sender.send(WorkerStreamEvent::Output {
                line,
                timestamp: Instant::now(),
                is_stderr,
            });
        }
    }

    /// Emit a progress update
    async fn emit_progress(&self, message: String, percentage: Option<f32>) {
        if let Some(ref sender) = self.event_sender {
            let _ = sender.send(WorkerStreamEvent::Progress {
                message,
                percentage,
                timestamp: Instant::now(),
            });
        }
    }

    /// Emit state change
    async fn set_state(&mut self, new_state: WorkerState) {
        let old_state = self.state.clone();
        self.state = new_state.clone();

        if let Some(ref sender) = self.event_sender {
            let _ = sender.send(WorkerStreamEvent::StateChanged {
                old_state,
                new_state,
                timestamp: Instant::now(),
            });
        }
    }

    /// Emit completion event
    async fn emit_completion(&self, success: bool, output: &str) {
        if let Some(ref sender) = self.event_sender {
            let duration = self.started_at
                .map(|start| Instant::now().duration_since(start))
                .unwrap_or_default();

            let _ = sender.send(WorkerStreamEvent::Completed {
                success,
                final_output: output.to_string(),
                duration,
                timestamp: Instant::now(),
            });
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
            streaming_enabled: self.streaming_config.enabled,
            lines_processed: self.lines_processed,
            started_at: self.started_at,
        }
    }

    /// Cancel the worker
    pub async fn cancel(&mut self) -> Result<()> {
        if let Some(mut process) = self.process_handle.take() {
            process.kill().await.context("Failed to kill process")?;
        }
        self.set_state(WorkerState::Cancelled).await;
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

    /// Get lines processed count
    pub fn lines_processed(&self) -> usize {
        self.lines_processed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::TaskStatus;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_streaming_worker_creation() {
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
        let worker = StreamingWorker::new(
            task,
            workspace.path().to_path_buf(),
            WorkerKind::ClaudeCode,
            "claude".to_string(),
        )
        .unwrap();

        assert_eq!(worker.kind, WorkerKind::ClaudeCode);
        assert!(matches!(worker.state, WorkerState::Initializing));
        assert_eq!(worker.lines_processed, 0);
    }

    #[test]
    fn test_percentage_extraction() {
        let worker = StreamingWorker {
            task: Task {
                id: "test".to_string(),
                description: "test".to_string(),
                instructions: "test".to_string(),
                relevant_files: vec![],
                dependencies: vec![],
                preferred_worker: None,
                priority: 0,
                status: TaskStatus::Pending,
            },
            workspace: std::env::temp_dir(),
            kind: WorkerKind::ClaudeCode,
            cli_path: "claude".to_string(),
            state: WorkerState::Initializing,
            output: String::new(),
            process_handle: None,
            streaming_config: StreamingConfig::default(),
            event_sender: None,
            started_at: None,
            lines_processed: 0,
        };

        assert_eq!(worker.extract_percentage("Progress: 45%"), Some(45.0));
        assert_eq!(worker.extract_percentage("Building: 87.5%"), Some(87.5));
        assert_eq!(worker.extract_percentage("No percentage here"), None);
    }

    #[tokio::test]
    async fn test_event_streaming() {
        let task = Task {
            id: "test-1".to_string(),
            description: "Test task".to_string(),
            instructions: "echo 'test'".to_string(),
            relevant_files: vec![],
            dependencies: vec![],
            preferred_worker: None,
            priority: 0,
            status: TaskStatus::Pending,
        };

        let workspace = tempdir().unwrap();
        let (mut worker, mut event_rx) = StreamingWorker::new(
            task,
            workspace.path().to_path_buf(),
            WorkerKind::ClaudeCode,
            "claude".to_string(),
        )
        .unwrap()
        .with_event_stream();

        // Test emitting an output line
        worker.emit_output_line("Test line".to_string(), false).await;

        // Should receive the event
        let event = event_rx.recv().await.unwrap();
        match event {
            WorkerStreamEvent::Output { line, is_stderr, .. } => {
                assert_eq!(line, "Test line");
                assert!(!is_stderr);
            }
            _ => panic!("Expected Output event"),
        }
    }
}