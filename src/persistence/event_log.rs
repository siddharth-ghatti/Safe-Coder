//! JSONL Event Logging for Crash-Safe Session Recording
//!
//! This module provides append-only JSONL logging for session events,
//! similar to Codex's approach. Events are streamed to disk immediately,
//! providing crash recovery and real-time session persistence.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

use crate::llm::Message;

/// Event types that can be logged to the session JSONL file
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionLogEvent {
    /// Session started
    SessionStart {
        session_id: String,
        project_path: String,
        timestamp: DateTime<Utc>,
        model: String,
    },
    /// User message added
    UserMessage {
        timestamp: DateTime<Utc>,
        content: String,
    },
    /// Assistant message added
    AssistantMessage {
        timestamp: DateTime<Utc>,
        content: String,
    },
    /// Full message with content blocks (for tool calls)
    Message {
        timestamp: DateTime<Utc>,
        message: Message,
    },
    /// Tool was executed
    ToolExecution {
        timestamp: DateTime<Utc>,
        tool_name: String,
        tool_id: String,
        input: serde_json::Value,
    },
    /// Tool result received
    ToolResult {
        timestamp: DateTime<Utc>,
        tool_id: String,
        success: bool,
        output: String,
    },
    /// Context was compacted
    ContextCompaction {
        timestamp: DateTime<Utc>,
        messages_before: usize,
        messages_after: usize,
        tokens_before: usize,
        tokens_after: usize,
        compaction_count: usize,
    },
    /// Token usage update
    TokenUsage {
        timestamp: DateTime<Utc>,
        input_tokens: usize,
        output_tokens: usize,
        cache_read_tokens: Option<usize>,
        cache_creation_tokens: Option<usize>,
        cumulative_input: usize,
        cumulative_output: usize,
    },
    /// Session ended
    SessionEnd {
        timestamp: DateTime<Utc>,
        reason: String,
        total_messages: usize,
        total_tokens: usize,
    },
    /// Error occurred
    Error {
        timestamp: DateTime<Utc>,
        error: String,
        context: Option<String>,
    },
    /// Checkpoint created
    CheckpointCreated {
        timestamp: DateTime<Utc>,
        checkpoint_id: String,
        label: Option<String>,
    },
    /// Session resumed
    SessionResumed {
        timestamp: DateTime<Utc>,
        original_session_id: String,
        messages_restored: usize,
    },
}

/// Header written as the first line of each JSONL session file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLogHeader {
    pub version: String,
    pub session_id: String,
    pub project_path: String,
    pub model: String,
    pub created_at: DateTime<Utc>,
}

/// Append-only JSONL event logger for session events
pub struct EventLogger {
    session_id: String,
    log_path: PathBuf,
    file: Option<File>,
    cumulative_input_tokens: usize,
    cumulative_output_tokens: usize,
    compaction_count: usize,
}

impl EventLogger {
    /// Create a new event logger for a session
    /// Sessions are stored in ~/.config/safe-coder/sessions/YYYY/MM/DD/
    pub fn new(session_id: String, project_path: &PathBuf, model: &str) -> Result<Self> {
        let log_path = Self::session_log_path(&session_id)?;

        // Create parent directories
        if let Some(parent) = log_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Open file for appending
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .context("Failed to open session log file")?;

        let mut logger = Self {
            session_id: session_id.clone(),
            log_path,
            file: Some(file),
            cumulative_input_tokens: 0,
            cumulative_output_tokens: 0,
            compaction_count: 0,
        };

        // Write header as first line (only if file is empty/new)
        let metadata = fs::metadata(&logger.log_path)?;
        if metadata.len() == 0 {
            let header = SessionLogHeader {
                version: "1.0".to_string(),
                session_id,
                project_path: project_path.to_string_lossy().to_string(),
                model: model.to_string(),
                created_at: Utc::now(),
            };
            logger.write_line(&serde_json::to_string(&header)?)?;
        }

        Ok(logger)
    }

    /// Resume an existing session log
    pub fn resume(session_id: &str) -> Result<Self> {
        let log_path = Self::session_log_path(session_id)?;

        if !log_path.exists() {
            anyhow::bail!("Session log not found: {}", session_id);
        }

        // Open file for appending
        let file = OpenOptions::new()
            .append(true)
            .open(&log_path)
            .context("Failed to open session log file")?;

        // Count existing token usage
        let (cumulative_input, cumulative_output, compaction_count) =
            Self::count_tokens_from_log(&log_path)?;

        Ok(Self {
            session_id: session_id.to_string(),
            log_path,
            file: Some(file),
            cumulative_input_tokens: cumulative_input,
            cumulative_output_tokens: cumulative_output,
            compaction_count,
        })
    }

    /// Get the path for a session log file
    fn session_log_path(session_id: &str) -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("Failed to get config directory")?;
        let now = Utc::now();

        Ok(config_dir
            .join("safe-coder")
            .join("sessions")
            .join(now.format("%Y").to_string())
            .join(now.format("%m").to_string())
            .join(now.format("%d").to_string())
            .join(format!("session-{}.jsonl", session_id)))
    }

    /// Get session logs directory
    pub fn sessions_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("Failed to get config directory")?;
        Ok(config_dir.join("safe-coder").join("sessions"))
    }

    /// Write a line to the log file
    fn write_line(&mut self, line: &str) -> Result<()> {
        if let Some(ref mut file) = self.file {
            writeln!(file, "{}", line)?;
            file.flush()?;
        }
        Ok(())
    }

    /// Log an event
    pub fn log(&mut self, event: SessionLogEvent) -> Result<()> {
        // Update cumulative counters
        if let SessionLogEvent::TokenUsage {
            input_tokens,
            output_tokens,
            ..
        } = &event
        {
            self.cumulative_input_tokens += input_tokens;
            self.cumulative_output_tokens += output_tokens;
        }
        if let SessionLogEvent::ContextCompaction { .. } = &event {
            self.compaction_count += 1;
        }

        let json = serde_json::to_string(&event)?;
        self.write_line(&json)
    }

    /// Log a user message
    pub fn log_user_message(&mut self, content: &str) -> Result<()> {
        self.log(SessionLogEvent::UserMessage {
            timestamp: Utc::now(),
            content: content.to_string(),
        })
    }

    /// Log an assistant message
    pub fn log_assistant_message(&mut self, content: &str) -> Result<()> {
        self.log(SessionLogEvent::AssistantMessage {
            timestamp: Utc::now(),
            content: content.to_string(),
        })
    }

    /// Log a full message with content blocks
    pub fn log_message(&mut self, message: &Message) -> Result<()> {
        self.log(SessionLogEvent::Message {
            timestamp: Utc::now(),
            message: message.clone(),
        })
    }

    /// Log tool execution
    pub fn log_tool_execution(
        &mut self,
        tool_name: &str,
        tool_id: &str,
        input: serde_json::Value,
    ) -> Result<()> {
        self.log(SessionLogEvent::ToolExecution {
            timestamp: Utc::now(),
            tool_name: tool_name.to_string(),
            tool_id: tool_id.to_string(),
            input,
        })
    }

    /// Log tool result
    pub fn log_tool_result(&mut self, tool_id: &str, success: bool, output: &str) -> Result<()> {
        self.log(SessionLogEvent::ToolResult {
            timestamp: Utc::now(),
            tool_id: tool_id.to_string(),
            success,
            output: output.to_string(),
        })
    }

    /// Log token usage
    pub fn log_token_usage(
        &mut self,
        input_tokens: usize,
        output_tokens: usize,
        cache_read_tokens: Option<usize>,
        cache_creation_tokens: Option<usize>,
    ) -> Result<()> {
        self.cumulative_input_tokens += input_tokens;
        self.cumulative_output_tokens += output_tokens;

        self.log(SessionLogEvent::TokenUsage {
            timestamp: Utc::now(),
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_creation_tokens,
            cumulative_input: self.cumulative_input_tokens,
            cumulative_output: self.cumulative_output_tokens,
        })
    }

    /// Log context compaction
    pub fn log_compaction(
        &mut self,
        messages_before: usize,
        messages_after: usize,
        tokens_before: usize,
        tokens_after: usize,
    ) -> Result<()> {
        self.compaction_count += 1;
        self.log(SessionLogEvent::ContextCompaction {
            timestamp: Utc::now(),
            messages_before,
            messages_after,
            tokens_before,
            tokens_after,
            compaction_count: self.compaction_count,
        })
    }

    /// Log session end
    pub fn log_session_end(&mut self, reason: &str, total_messages: usize) -> Result<()> {
        self.log(SessionLogEvent::SessionEnd {
            timestamp: Utc::now(),
            reason: reason.to_string(),
            total_messages,
            total_tokens: self.cumulative_input_tokens + self.cumulative_output_tokens,
        })
    }

    /// Log an error
    pub fn log_error(&mut self, error: &str, context: Option<&str>) -> Result<()> {
        self.log(SessionLogEvent::Error {
            timestamp: Utc::now(),
            error: error.to_string(),
            context: context.map(|s| s.to_string()),
        })
    }

    /// Get session ID
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get log path
    pub fn log_path(&self) -> &PathBuf {
        &self.log_path
    }

    /// Get compaction count
    pub fn compaction_count(&self) -> usize {
        self.compaction_count
    }

    /// Count tokens from an existing log file
    fn count_tokens_from_log(log_path: &PathBuf) -> Result<(usize, usize, usize)> {
        let file = File::open(log_path)?;
        let reader = BufReader::new(file);

        let mut cumulative_input = 0;
        let mut cumulative_output = 0;
        let mut compaction_count = 0;

        for line in reader.lines().skip(1) {
            // Skip header
            let line = line?;
            if let Ok(event) = serde_json::from_str::<SessionLogEvent>(&line) {
                match event {
                    SessionLogEvent::TokenUsage {
                        cumulative_input: ci,
                        cumulative_output: co,
                        ..
                    } => {
                        cumulative_input = ci;
                        cumulative_output = co;
                    }
                    SessionLogEvent::ContextCompaction {
                        compaction_count: cc,
                        ..
                    } => {
                        compaction_count = cc;
                    }
                    _ => {}
                }
            }
        }

        Ok((cumulative_input, cumulative_output, compaction_count))
    }

    /// Load messages from a session log file
    pub fn load_messages(session_id: &str) -> Result<Vec<Message>> {
        let log_path = Self::session_log_path(session_id)?;

        if !log_path.exists() {
            anyhow::bail!("Session log not found: {}", session_id);
        }

        let file = File::open(&log_path)?;
        let reader = BufReader::new(file);

        let mut messages = Vec::new();

        for line in reader.lines().skip(1) {
            // Skip header
            let line = line?;
            if let Ok(event) = serde_json::from_str::<SessionLogEvent>(&line) {
                if let SessionLogEvent::Message { message, .. } = event {
                    messages.push(message);
                }
            }
        }

        Ok(messages)
    }

    /// List recent sessions (last N days)
    pub fn list_recent_sessions(days: usize) -> Result<Vec<SessionInfo>> {
        let sessions_dir = Self::sessions_dir()?;
        let mut sessions = Vec::new();

        if !sessions_dir.exists() {
            return Ok(sessions);
        }

        // Walk through date directories
        let now = Utc::now();
        for day_offset in 0..days {
            let date = now - chrono::Duration::days(day_offset as i64);
            let day_dir = sessions_dir
                .join(date.format("%Y").to_string())
                .join(date.format("%m").to_string())
                .join(date.format("%d").to_string());

            if !day_dir.exists() {
                continue;
            }

            for entry in fs::read_dir(&day_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().map_or(false, |e| e == "jsonl") {
                    if let Some(info) = Self::parse_session_file(&path)? {
                        sessions.push(info);
                    }
                }
            }
        }

        // Sort by created_at descending
        sessions.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(sessions)
    }

    /// Parse session info from a log file
    fn parse_session_file(path: &PathBuf) -> Result<Option<SessionInfo>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        if let Some(Ok(first_line)) = reader.lines().next() {
            if let Ok(header) = serde_json::from_str::<SessionLogHeader>(&first_line) {
                // Count events
                let file = File::open(path)?;
                let reader = BufReader::new(file);
                let event_count = reader.lines().count() - 1; // Subtract header

                return Ok(Some(SessionInfo {
                    session_id: header.session_id,
                    project_path: header.project_path,
                    model: header.model,
                    created_at: header.created_at,
                    log_path: path.clone(),
                    event_count,
                }));
            }
        }

        Ok(None)
    }

    /// Get the most recent session
    pub fn get_last_session() -> Result<Option<SessionInfo>> {
        let sessions = Self::list_recent_sessions(7)?;
        Ok(sessions.into_iter().next())
    }
}

/// Information about a session
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub session_id: String,
    pub project_path: String,
    pub model: String,
    pub created_at: DateTime<Utc>,
    pub log_path: PathBuf,
    pub event_count: usize,
}

impl SessionInfo {
    /// Format for display
    pub fn display(&self) -> String {
        format!(
            "{} | {} | {} events | {}",
            self.session_id,
            self.created_at.format("%Y-%m-%d %H:%M"),
            self.event_count,
            self.project_path
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_event_serialization() {
        let event = SessionLogEvent::UserMessage {
            timestamp: Utc::now(),
            content: "Hello, world!".to_string(),
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("user_message"));
        assert!(json.contains("Hello, world!"));

        let parsed: SessionLogEvent = serde_json::from_str(&json).unwrap();
        if let SessionLogEvent::UserMessage { content, .. } = parsed {
            assert_eq!(content, "Hello, world!");
        } else {
            panic!("Wrong event type");
        }
    }

    #[test]
    fn test_token_usage_event() {
        let event = SessionLogEvent::TokenUsage {
            timestamp: Utc::now(),
            input_tokens: 100,
            output_tokens: 50,
            cache_read_tokens: Some(20),
            cache_creation_tokens: Some(10),
            cumulative_input: 500,
            cumulative_output: 250,
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: SessionLogEvent = serde_json::from_str(&json).unwrap();

        if let SessionLogEvent::TokenUsage {
            cumulative_input,
            cumulative_output,
            ..
        } = parsed
        {
            assert_eq!(cumulative_input, 500);
            assert_eq!(cumulative_output, 250);
        } else {
            panic!("Wrong event type");
        }
    }
}
