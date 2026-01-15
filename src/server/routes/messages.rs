//! Message handling endpoints

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use tokio::sync::mpsc;

use crate::server::state::AppState;
use crate::server::types::{ErrorResponse, MessageDto, SendMessageRequest, ServerEvent};
use crate::session::SessionEvent;

/// GET /api/sessions/:id/messages - Get message history
pub async fn get_messages(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<Vec<MessageDto>>, (StatusCode, Json<ErrorResponse>)> {
    let sessions = state.sessions.read().await;

    match sessions.get(&session_id) {
        Some(handle) => {
            let session = handle.session.read().await;
            let messages = session.get_messages();

            let dtos: Vec<MessageDto> = messages
                .iter()
                .enumerate()
                .filter_map(|(i, msg)| {
                    // Extract text content
                    let content = msg
                        .content
                        .iter()
                        .filter_map(|block| {
                            if let crate::llm::ContentBlock::Text { text } = block {
                                Some(text.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    // Skip messages with no text content
                    if content.trim().is_empty() {
                        return None;
                    }

                    Some(MessageDto {
                        id: format!("msg_{}", i),
                        role: format!("{:?}", msg.role).to_lowercase(),
                        content,
                        timestamp: chrono::Utc::now().to_rfc3339(), // TODO: store timestamps
                        tool_calls: None, // TODO: extract tool calls
                    })
                })
                .collect();

            Ok(Json(dtos))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Session not found: {}", session_id),
                code: "SESSION_NOT_FOUND".to_string(),
            }),
        )),
    }
}

/// POST /api/sessions/:id/messages - Send a message
pub async fn send_message(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<SendMessageRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Check if session exists
    let handle = {
        let sessions = state.sessions.read().await;
        sessions.get(&session_id).cloned()
    };

    let handle = match handle {
        Some(h) => h,
        None => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Session not found: {}", session_id),
                    code: "SESSION_NOT_FOUND".to_string(),
                }),
            ))
        }
    };

    // Check if already processing
    {
        let is_processing = handle.is_processing.read().await;
        if *is_processing {
            return Err((
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: "Session is already processing a message".to_string(),
                    code: "SESSION_BUSY".to_string(),
                }),
            ));
        }
    }

    // Mark as processing
    {
        let mut is_processing = handle.is_processing.write().await;
        *is_processing = true;
    }

    // Get event sender
    let event_sender = state.get_event_sender(&session_id).await;

    // Broadcast connected event
    let _ = event_sender.send(ServerEvent::Connected);

    // Create a channel for session events
    let (session_tx, mut session_rx) = mpsc::unbounded_channel::<SessionEvent>();

    // Spawn task to forward session events to broadcast channel
    let event_sender_clone = event_sender.clone();
    let session_id_clone = session_id.clone();
    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        tracing::info!("Event forwarding task started for session {}", session_id_clone);
        while let Some(event) = session_rx.recv().await {
            tracing::debug!("Forwarding event: {:?}", std::mem::discriminant(&event));

            // Handle file diff events specially to track changes
            if let SessionEvent::FileDiff { ref path, ref old_content, ref new_content } = event {
                tracing::info!("FileDiff event received for path: {}", path);
                // Compute diff stats
                let diff = similar::TextDiff::from_lines(old_content, new_content);
                let (additions, deletions) = diff.iter_all_changes().fold((0, 0), |(a, d), c| {
                    match c.tag() {
                        similar::ChangeTag::Insert => (a + 1, d),
                        similar::ChangeTag::Delete => (a, d + 1),
                        _ => (a, d),
                    }
                });

                // Track file change
                state_clone.add_file_change(
                    &session_id_clone,
                    crate::server::state::FileChange {
                        path: path.clone(),
                        change_type: if old_content.is_empty() {
                            crate::server::state::FileChangeType::Created
                        } else {
                            crate::server::state::FileChangeType::Modified
                        },
                        additions,
                        deletions,
                        timestamp: chrono::Utc::now(),
                        old_content: Some(old_content.clone()),
                        new_content: Some(new_content.clone()),
                    },
                ).await;
            }

            // Handle doom loop prompts - register response channel with state
            if let SessionEvent::DoomLoopPrompt { ref prompt_id, ref response_tx, .. } = event {
                tracing::info!("DoomLoopPrompt event received, registering response channel: {}", prompt_id);
                state_clone.register_doom_loop_response(prompt_id.clone(), response_tx.clone()).await;
            }

            // Convert and broadcast
            let server_event: ServerEvent = event.into();
            let _ = event_sender_clone.send(server_event);
        }
    });

    // Send message to session (in background)
    let handle_clone = handle.clone();
    let message = request.content.clone();
    let session_id_for_log = session_id.clone();
    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        tracing::info!("Processing message for session {}", session_id_for_log);

        let mut session = handle_clone.session.write().await;
        tracing::info!("Got session lock, sending message...");

        // Use send_message_with_progress for proper build/plan mode handling
        // This bypasses unified planning and uses direct execution like the TUI
        tracing::info!("Calling send_message_with_progress, agent_mode: {:?}", session.agent_mode());
        match session.send_message_with_progress(message, session_tx).await {
            Ok(response) => {
                tracing::info!("Message processed successfully, response length: {}", response.len());

                // Save messages to persistent storage
                if let Some(persistence) = state_clone.persistence() {
                    let messages = session.get_messages();
                    if let Err(e) = persistence.update_session(&session_id_for_log, &messages).await {
                        tracing::warn!("Failed to persist messages: {}", e);
                    } else {
                        tracing::debug!("Messages persisted to SQLite");
                    }
                }
            }
            Err(e) => {
                tracing::error!("Failed to process message: {}", e);
                let _ = event_sender.send(ServerEvent::Error {
                    message: format!("Failed to process message: {}", e)
                });
            }
        }

        // Mark as not processing
        {
            let mut is_processing = handle_clone.is_processing.write().await;
            *is_processing = false;
        }

        // Broadcast completion
        let _ = event_sender.send(ServerEvent::Completed);
        tracing::info!("Message processing completed for session {}", session_id_for_log);
    });

    Ok(Json(serde_json::json!({
        "status": "processing",
        "message": "Message sent, subscribe to /api/sessions/{}/events for updates",
    })))
}

/// POST /api/sessions/:id/cancel - Cancel current operation
pub async fn cancel_operation(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let sessions = state.sessions.read().await;

    match sessions.get(&session_id) {
        Some(handle) => {
            // Mark as not processing (this is a simple cancel)
            let mut is_processing = handle.is_processing.write().await;
            *is_processing = false;

            // Broadcast cancellation
            let _ = state
                .get_event_sender(&session_id)
                .await
                .send(ServerEvent::Error {
                    message: "Operation cancelled".to_string(),
                });

            Ok(Json(serde_json::json!({
                "status": "cancelled"
            })))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Session not found: {}", session_id),
                code: "SESSION_NOT_FOUND".to_string(),
            }),
        )),
    }
}
