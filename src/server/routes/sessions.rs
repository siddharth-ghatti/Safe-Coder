//! Session management endpoints

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use tokio::sync::RwLock;

use crate::approval::UserMode;
use crate::config::Config;
use crate::server::state::{AppState, SessionHandle};
use crate::server::types::{
    CreateSessionRequest, ErrorResponse, FileChangeStats, SessionListResponse,
    SessionResponse, SessionSummary,
};
use crate::session::Session;

/// GET /api/sessions - List all active sessions
pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
) -> Json<SessionListResponse> {
    let sessions = state.sessions.read().await;

    let mut summaries = Vec::new();
    for (id, handle) in sessions.iter() {
        let file_changes = handle.file_changes.read().await;
        let (additions, deletions) = file_changes
            .iter()
            .fold((0, 0), |(a, d), c| (a + c.additions, d + c.deletions));

        summaries.push(SessionSummary {
            id: id.clone(),
            project_path: handle.project_path.display().to_string(),
            created_at: handle.created_at.to_rfc3339(),
            message_count: 0, // TODO: get from session
            file_changes: FileChangeStats {
                total_files: file_changes.len(),
                additions,
                deletions,
            },
        });
    }

    Json(SessionListResponse { sessions: summaries })
}

/// POST /api/sessions - Create a new session
pub async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<SessionResponse>), (StatusCode, Json<ErrorResponse>)> {
    // Validate project path
    let project_path = PathBuf::from(&request.project_path);
    let canonical_path = project_path.canonicalize().map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid project path: {}", e),
                code: "INVALID_PATH".to_string(),
            }),
        )
    })?;

    // Load config and disable git auto-commit (user can use bash for git operations)
    let mut config = Config::load().unwrap_or_default();
    config.git.auto_commit = false;

    // Create session
    let mut session = Session::new(config, canonical_path.clone())
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to create session: {}", e),
                    code: "SESSION_CREATE_FAILED".to_string(),
                }),
            )
        })?;

    // Set user mode based on request (defaults to Build mode)
    let mode_str = request.mode.as_deref().unwrap_or("build");
    let user_mode = match mode_str.to_lowercase().as_str() {
        "plan" => UserMode::Plan,
        _ => UserMode::Build,
    };
    session.set_user_mode(user_mode);

    // Generate session ID
    let session_id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now();

    // Create session handle
    let handle = SessionHandle {
        session: Arc::new(RwLock::new(session)),
        project_path: canonical_path.clone(),
        created_at,
        is_processing: Arc::new(RwLock::new(false)),
        file_changes: Arc::new(RwLock::new(Vec::new())),
    };

    // Store session
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id.clone(), handle);
    }

    // Create event channel for this session
    let _ = state.get_event_sender(&session_id).await;

    Ok((
        StatusCode::CREATED,
        Json(SessionResponse {
            id: session_id,
            project_path: canonical_path.display().to_string(),
            created_at: created_at.to_rfc3339(),
            mode: request.mode.unwrap_or_else(|| "build".to_string()),
        }),
    ))
}

/// GET /api/sessions/:id - Get session details
pub async fn get_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<SessionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let sessions = state.sessions.read().await;

    match sessions.get(&session_id) {
        Some(handle) => Ok(Json(SessionResponse {
            id: session_id,
            project_path: handle.project_path.display().to_string(),
            created_at: handle.created_at.to_rfc3339(),
            mode: "build".to_string(),
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Session not found: {}", session_id),
                code: "SESSION_NOT_FOUND".to_string(),
            }),
        )),
    }
}

/// DELETE /api/sessions/:id - Delete a session
pub async fn delete_session(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let mut sessions = state.sessions.write().await;

    if sessions.remove(&session_id).is_some() {
        // Also remove event channel
        let mut channels = state.event_channels.write().await;
        channels.remove(&session_id);

        Ok(StatusCode::NO_CONTENT)
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Session not found: {}", session_id),
                code: "SESSION_NOT_FOUND".to_string(),
            }),
        ))
    }
}
