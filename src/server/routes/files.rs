//! File operations endpoints

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::server::state::AppState;
use crate::server::types::{ErrorResponse, FileChangeDto, FileChangeStats, FileChangesResponse};

#[derive(Debug, Deserialize)]
pub struct ListFilesQuery {
    pub query: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct ProjectFile {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Serialize)]
pub struct ListFilesResponse {
    pub files: Vec<ProjectFile>,
}

/// GET /api/sessions/:id/files - List files in project directory for @ mentions
pub async fn list_project_files(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Query(query): Query<ListFilesQuery>,
) -> Result<Json<ListFilesResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if session exists and get project path
    let sessions = state.sessions.read().await;
    let session = sessions.get(&session_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Session not found: {}", session_id),
                code: "SESSION_NOT_FOUND".to_string(),
            }),
        )
    })?;

    let project_path = session.project_path.display().to_string();
    drop(sessions);

    let search_query = query.query.unwrap_or_default().to_lowercase();
    let limit = query.limit.unwrap_or(50);

    // Walk directory and collect matching files
    let mut files: Vec<ProjectFile> = Vec::new();

    for entry in WalkDir::new(&project_path)
        .max_depth(10)
        .into_iter()
        .filter_entry(|e| {
            // Skip hidden files and common ignore patterns
            let name = e.file_name().to_string_lossy();
            !name.starts_with('.')
                && name != "node_modules"
                && name != "target"
                && name != "dist"
                && name != "build"
                && name != "__pycache__"
                && name != ".git"
        })
    {
        if files.len() >= limit {
            break;
        }

        if let Ok(entry) = entry {
            let path = entry.path();
            let relative_path = path
                .strip_prefix(&project_path)
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();

            // Skip root directory
            if relative_path.is_empty() {
                continue;
            }

            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().is_dir();

            // Filter by search query if provided
            if !search_query.is_empty() {
                if !relative_path.to_lowercase().contains(&search_query)
                    && !name.to_lowercase().contains(&search_query)
                {
                    continue;
                }
            }

            // Prefer files over directories for @ mentions
            if !is_dir {
                files.push(ProjectFile {
                    path: relative_path,
                    name,
                    is_dir,
                });
            }
        }
    }

    // Sort by path length (shorter = more relevant)
    files.sort_by(|a, b| a.path.len().cmp(&b.path.len()));

    Ok(Json(ListFilesResponse { files }))
}

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
