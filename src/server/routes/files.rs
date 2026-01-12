//! File operations endpoints

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::server::state::AppState;
use crate::server::types::{ErrorResponse, FileChangeDto, FileChangeStats, FileChangesResponse};

/// GET /api/sessions/:id/changes - Get file changes for a session
pub async fn get_session_changes(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<FileChangesResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if session exists
    if !state.session_exists(&session_id).await {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Session not found: {}", session_id),
                code: "SESSION_NOT_FOUND".to_string(),
            }),
        ));
    }

    // Get file changes
    let changes = state.get_file_changes(&session_id).await;

    // Calculate stats
    let (total_additions, total_deletions) = changes
        .iter()
        .fold((0, 0), |(a, d), c| (a + c.additions, d + c.deletions));

    // Convert to DTOs
    let change_dtos: Vec<FileChangeDto> = changes
        .into_iter()
        .map(|c| {
            // Generate diff if we have content
            let diff = match (&c.old_content, &c.new_content) {
                (Some(old), Some(new)) => {
                    let text_diff = similar::TextDiff::from_lines(old, new);
                    Some(
                        text_diff
                            .unified_diff()
                            .context_radius(3)
                            .header(&c.path, &c.path)
                            .to_string(),
                    )
                }
                _ => None,
            };

            FileChangeDto {
                path: c.path,
                change_type: format!("{:?}", c.change_type).to_lowercase(),
                additions: c.additions,
                deletions: c.deletions,
                timestamp: c.timestamp.to_rfc3339(),
                diff,
            }
        })
        .collect();

    Ok(Json(FileChangesResponse {
        session_id,
        changes: change_dtos.clone(),
        stats: FileChangeStats {
            total_files: change_dtos.len(),
            additions: total_additions,
            deletions: total_deletions,
        },
    }))
}
