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

/// GET /api/sessions/:id/changes - Get file changes for a session using git
pub async fn get_session_changes(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
) -> Result<Json<FileChangesResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Get project path from session
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
    let project_path = session.project_path.clone();
    drop(sessions);

    // Use git to get file changes
    let change_dtos = get_git_changes(&project_path).await;

    // Calculate stats
    let (total_additions, total_deletions): (i32, i32) = change_dtos
        .iter()
        .fold((0, 0), |(a, d), c| (a + c.additions, d + c.deletions));

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

/// Get file changes from git status and diff
async fn get_git_changes(project_path: &std::path::Path) -> Vec<FileChangeDto> {
    use std::process::Command;

    let mut changes = Vec::new();

    // Run git status --porcelain to get changed files
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(project_path)
        .output();

    let status_output = match status_output {
        Ok(output) => output,
        Err(e) => {
            tracing::warn!("Failed to run git status: {}", e);
            return changes;
        }
    };

    if !status_output.status.success() {
        tracing::warn!("git status failed - directory may not be a git repo");
        return changes;
    }

    let status_str = String::from_utf8_lossy(&status_output.stdout);

    for line in status_str.lines() {
        if line.len() < 3 {
            continue;
        }

        let status_code = &line[0..2];
        let file_path = line[3..].trim();

        // Determine change type from git status code
        let change_type = match status_code.trim() {
            "M" | " M" | "MM" => "modified",
            "A" | " A" | "AM" => "created",
            "D" | " D" => "deleted",
            "??" => "created", // Untracked files
            "R" | " R" => "modified", // Renamed
            _ => "modified",
        };

        // Get diff for this file
        let (additions, deletions, diff) = get_file_diff(project_path, file_path, change_type);

        changes.push(FileChangeDto {
            path: file_path.to_string(),
            change_type: change_type.to_string(),
            additions: additions as i32,
            deletions: deletions as i32,
            timestamp: chrono::Utc::now().to_rfc3339(),
            diff,
        });
    }

    changes
}

/// Get diff for a specific file
fn get_file_diff(project_path: &std::path::Path, file_path: &str, change_type: &str) -> (usize, usize, Option<String>) {
    use std::process::Command;

    // For untracked files, show the entire content as additions
    if change_type == "created" {
        let full_path = project_path.join(file_path);
        if let Ok(content) = std::fs::read_to_string(&full_path) {
            let line_count = content.lines().count();
            let diff = format!(
                "--- /dev/null\n+++ b/{}\n@@ -0,0 +1,{} @@\n{}",
                file_path,
                line_count,
                content.lines().map(|l| format!("+{}", l)).collect::<Vec<_>>().join("\n")
            );
            return (line_count, 0, Some(diff));
        }
    }

    // Run git diff for tracked files
    let diff_output = Command::new("git")
        .args(["diff", "--", file_path])
        .current_dir(project_path)
        .output();

    // Also try staged diff
    let staged_diff_output = Command::new("git")
        .args(["diff", "--staged", "--", file_path])
        .current_dir(project_path)
        .output();

    let diff_str = match (diff_output, staged_diff_output) {
        (Ok(unstaged), Ok(staged)) => {
            let unstaged_str = String::from_utf8_lossy(&unstaged.stdout);
            let staged_str = String::from_utf8_lossy(&staged.stdout);
            if !unstaged_str.is_empty() {
                unstaged_str.to_string()
            } else {
                staged_str.to_string()
            }
        }
        (Ok(output), _) => String::from_utf8_lossy(&output.stdout).to_string(),
        _ => String::new(),
    };

    if diff_str.is_empty() {
        return (0, 0, None);
    }

    // Count additions and deletions
    let mut additions = 0;
    let mut deletions = 0;
    for line in diff_str.lines() {
        if line.starts_with('+') && !line.starts_with("+++") {
            additions += 1;
        } else if line.starts_with('-') && !line.starts_with("---") {
            deletions += 1;
        }
    }

    (additions, deletions, Some(diff_str))
}
