//! Server-Sent Events (SSE) endpoint for real-time updates

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{Path, State},
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::Stream;
use tokio_stream::StreamExt;

use crate::server::state::AppState;
use crate::server::types::ServerEvent;

/// GET /api/sessions/:id/events - SSE event stream
pub async fn session_events(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Subscribe to events for this session
    let rx = state.subscribe_events(&session_id).await;

    // Convert broadcast receiver to stream
    let stream = tokio_stream::wrappers::BroadcastStream::new(rx)
        .filter_map(|result| {
            match result {
                Ok(event) => Some(event),
                Err(_) => None, // Skip lagged messages
            }
        })
        .map(|event: ServerEvent| {
            // Serialize event to JSON
            let json = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());

            // Create SSE event with type and data
            let event_type = match &event {
                ServerEvent::Connected => "Connected",
                ServerEvent::Thinking { .. } => "Thinking",
                ServerEvent::Reasoning { .. } => "Reasoning",
                ServerEvent::ToolStart { .. } => "ToolStart",
                ServerEvent::ToolOutput { .. } => "ToolOutput",
                ServerEvent::BashOutputLine { .. } => "BashOutputLine",
                ServerEvent::ToolComplete { .. } => "ToolComplete",
                ServerEvent::FileDiff { .. } => "FileDiff",
                ServerEvent::DiagnosticUpdate { .. } => "DiagnosticUpdate",
                ServerEvent::TextChunk { .. } => "TextChunk",
                ServerEvent::SubagentStarted { .. } => "SubagentStarted",
                ServerEvent::SubagentProgress { .. } => "SubagentProgress",
                ServerEvent::SubagentCompleted { .. } => "SubagentCompleted",
                ServerEvent::PlanCreated { .. } => "PlanCreated",
                ServerEvent::PlanStepStarted { .. } => "PlanStepStarted",
                ServerEvent::PlanStepCompleted { .. } => "PlanStepCompleted",
                ServerEvent::PlanAwaitingApproval { .. } => "PlanAwaitingApproval",
                ServerEvent::PlanApproved { .. } => "PlanApproved",
                ServerEvent::PlanRejected { .. } => "PlanRejected",
                ServerEvent::TokenUsage { .. } => "TokenUsage",
                ServerEvent::ContextCompressed { .. } => "ContextCompressed",
                ServerEvent::DoomLoopPrompt { .. } => "DoomLoopPrompt",
                ServerEvent::Error { .. } => "Error",
                ServerEvent::Completed => "Completed",
                ServerEvent::TodoList { .. } => "TodoList",
                ServerEvent::OrchestrateStarted { .. } => "OrchestrateStarted",
                ServerEvent::OrchestrateOutput { .. } => "OrchestrateOutput",
                ServerEvent::OrchestrateCompleted { .. } => "OrchestrateCompleted",
            };

            Ok::<_, Infallible>(
                Event::default()
                    .event(event_type)
                    .data(json)
            )
        });

    // Return SSE response with keep-alive
    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    )
}
