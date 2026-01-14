//! Server types and DTOs
//!
//! This module defines the types used for API requests, responses,
//! and server-sent events.

use serde::{Deserialize, Serialize};
use crate::session::SessionEvent;
use crate::planning::types::PlanEvent;

/// Server-sent event types for real-time updates
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ServerEvent {
    /// Connection established
    Connected,

    /// AI is thinking/processing
    Thinking { message: String },

    /// AI reasoning (before tool execution)
    Reasoning { text: String },

    /// Tool execution started
    ToolStart { name: String, description: String },

    /// Tool output (streaming or complete)
    ToolOutput { name: String, output: String },

    /// Bash output line (streaming)
    BashOutputLine { name: String, line: String },

    /// Tool execution completed
    ToolComplete { name: String, success: bool },

    /// File was changed
    FileDiff {
        path: String,
        additions: i32,
        deletions: i32,
        diff: String,
    },

    /// Diagnostic update (errors/warnings)
    DiagnosticUpdate { errors: usize, warnings: usize },

    /// Text chunk from AI response
    TextChunk { text: String },

    /// Subagent started
    SubagentStarted { id: String, kind: String, task: String },

    /// Subagent progress
    SubagentProgress { id: String, message: String },

    /// Subagent completed
    SubagentCompleted { id: String, success: bool, summary: String },

    /// Plan created
    PlanCreated { title: String, steps: Vec<PlanStepDto> },

    /// Plan step started
    PlanStepStarted { plan_id: String, step_id: String },

    /// Plan step completed
    PlanStepCompleted { plan_id: String, step_id: String, success: bool },

    /// Plan awaiting approval
    PlanAwaitingApproval { plan_id: String },

    /// Plan approved
    PlanApproved { plan_id: String },

    /// Plan rejected
    PlanRejected { plan_id: String },

    /// Token usage update
    TokenUsage {
        input_tokens: usize,
        output_tokens: usize,
        cache_read_tokens: Option<usize>,
        cache_creation_tokens: Option<usize>,
    },

    /// Context was compressed
    ContextCompressed { tokens_compressed: usize },

    /// Error occurred
    Error { message: String },

    /// Session completed (no more events)
    Completed,
}

/// Plan step DTO for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStepDto {
    pub id: String,
    pub description: String,
    pub status: String,
}

/// Convert SessionEvent to ServerEvent
impl From<SessionEvent> for ServerEvent {
    fn from(event: SessionEvent) -> Self {
        match event {
            SessionEvent::Thinking(message) => ServerEvent::Thinking { message },

            SessionEvent::Reasoning(text) => ServerEvent::Reasoning { text },

            SessionEvent::ToolStart { name, description } => {
                ServerEvent::ToolStart { name, description }
            }

            SessionEvent::ToolOutput { name, output } => {
                ServerEvent::ToolOutput { name, output }
            }

            SessionEvent::BashOutputLine { name, line } => {
                ServerEvent::BashOutputLine { name, line }
            }

            SessionEvent::ToolComplete { name, success } => {
                ServerEvent::ToolComplete { name, success }
            }

            SessionEvent::FileDiff { path, old_content, new_content } => {
                // Compute diff stats using similar crate
                let diff = similar::TextDiff::from_lines(&old_content, &new_content);
                let mut additions = 0;
                let mut deletions = 0;

                for change in diff.iter_all_changes() {
                    match change.tag() {
                        similar::ChangeTag::Insert => additions += 1,
                        similar::ChangeTag::Delete => deletions += 1,
                        _ => {}
                    }
                }

                // Generate unified diff string
                let diff_str = diff
                    .unified_diff()
                    .context_radius(3)
                    .header(&path, &path)
                    .to_string();

                ServerEvent::FileDiff {
                    path,
                    additions,
                    deletions,
                    diff: diff_str,
                }
            }

            SessionEvent::DiagnosticUpdate { errors, warnings } => {
                ServerEvent::DiagnosticUpdate { errors, warnings }
            }

            SessionEvent::TextChunk(text) => ServerEvent::TextChunk { text },

            SessionEvent::SubagentStarted { id, kind, task } => {
                ServerEvent::SubagentStarted { id, kind, task }
            }

            SessionEvent::SubagentProgress { id, message } => {
                ServerEvent::SubagentProgress { id, message }
            }

            SessionEvent::SubagentCompleted { id, success, summary } => {
                ServerEvent::SubagentCompleted { id, success, summary }
            }

            SessionEvent::Plan(plan_event) => {
                match plan_event {
                    PlanEvent::PlanCreated { plan } => {
                        let steps = plan.steps.iter().map(|s| PlanStepDto {
                            id: s.id.clone(),
                            description: s.description.clone(),
                            status: format!("{:?}", s.status),
                        }).collect();
                        ServerEvent::PlanCreated {
                            title: plan.title,
                            steps,
                        }
                    }
                    PlanEvent::StepStarted { plan_id, step_id, .. } => {
                        ServerEvent::PlanStepStarted { plan_id, step_id }
                    }
                    PlanEvent::StepCompleted { plan_id, step_id, success, .. } => {
                        ServerEvent::PlanStepCompleted { plan_id, step_id, success }
                    }
                    PlanEvent::AwaitingApproval { plan_id } => {
                        ServerEvent::PlanAwaitingApproval { plan_id }
                    }
                    PlanEvent::PlanApproved { plan_id } => {
                        ServerEvent::PlanApproved { plan_id }
                    }
                    PlanEvent::PlanRejected { plan_id } => {
                        ServerEvent::PlanRejected { plan_id }
                    }
                    _ => ServerEvent::Thinking { message: "Plan update".to_string() }
                }
            }

            SessionEvent::TokenUsage {
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_creation_tokens,
            } => ServerEvent::TokenUsage {
                input_tokens,
                output_tokens,
                cache_read_tokens,
                cache_creation_tokens,
            },

            SessionEvent::ContextCompressed { tokens_compressed } => {
                ServerEvent::ContextCompressed { tokens_compressed }
            }

            SessionEvent::CompactionWarning { message, .. } => {
                ServerEvent::Error { message }
            }

            // Handle the approval sender - we don't forward this directly
            SessionEvent::PlanApprovalSender(_) => {
                ServerEvent::Thinking { message: "Awaiting approval...".to_string() }
            }

            // Handle subagent tool usage
            SessionEvent::SubagentToolUsed { id, tool, description } => {
                ServerEvent::SubagentProgress {
                    id,
                    message: format!("Using {}: {}", tool, description),
                }
            }
        }
    }
}

// ============================================================================
// Request/Response DTOs
// ============================================================================

/// Request to create a new session
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub project_path: String,
    #[serde(default)]
    pub mode: Option<String>,
}

/// Request for changing session mode
#[derive(Debug, Deserialize)]
pub struct SetModeRequest {
    pub mode: String,
}

/// Response for session creation
#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub id: String,
    pub project_path: String,
    pub created_at: String,
    pub mode: String,
}

/// Request to send a message
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<AttachmentDto>,
}

/// Attachment DTO
#[derive(Debug, Deserialize)]
pub struct AttachmentDto {
    pub path: String,
    #[serde(default)]
    pub content: Option<String>,
}

/// Message response DTO
#[derive(Debug, Serialize)]
pub struct MessageDto {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallDto>>,
}

/// Tool call DTO
#[derive(Debug, Serialize)]
pub struct ToolCallDto {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

/// Session list response
#[derive(Debug, Serialize)]
pub struct SessionListResponse {
    pub sessions: Vec<SessionSummary>,
}

/// Session summary for list view
#[derive(Debug, Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub project_path: String,
    pub created_at: String,
    pub message_count: usize,
    pub file_changes: FileChangeStats,
}

/// File change statistics
#[derive(Debug, Serialize)]
pub struct FileChangeStats {
    pub total_files: usize,
    pub additions: i32,
    pub deletions: i32,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Config response
#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    pub provider: String,
    pub model: String,
    pub mode: String,
}

/// File changes response
#[derive(Debug, Serialize)]
pub struct FileChangesResponse {
    pub session_id: String,
    pub changes: Vec<FileChangeDto>,
    pub stats: FileChangeStats,
}

/// Individual file change DTO
#[derive(Debug, Clone, Serialize)]
pub struct FileChangeDto {
    pub path: String,
    pub change_type: String,
    pub additions: i32,
    pub deletions: i32,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff: Option<String>,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
}
