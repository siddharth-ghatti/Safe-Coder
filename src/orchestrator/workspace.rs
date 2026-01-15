//! Workspace manager for creating isolated git workspaces for each task

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;

use tokio::process::Command;

/// Manages git workspaces (worktrees or branches) for task isolation
pub struct WorkspaceManager {
    /// Base project path
    project_path: PathBuf,
    /// Base directory for worktrees
    worktree_base: PathBuf,
    /// Whether to use worktrees (vs just branches)
    use_worktrees: bool,
    /// Active workspaces: task_id -> workspace_path
    workspaces: HashMap<String, PathBuf>,
    /// Original branch name
    original_branch: Option<String>,
}

impl WorkspaceManager {
    /// Create a new workspace manager
    pub fn new(project_path: PathBuf, use_worktrees: bool) -> Result<Self> {
        // Create base directory for worktrees
        let worktree_base = project_path.join(".safe-coder-workspaces");

        Ok(Self {
            project_path,
            worktree_base,
            use_worktrees,
            workspaces: HashMap::new(),
            original_branch: None,
        })
    }

    /// Initialize the workspace manager (ensure git is set up)
    pub async fn init(&mut self) -> Result<()> {
        // Check if this is a git repository
        let git_check = Command::new("git")
            .current_dir(&self.project_path)
            .args(["rev-parse", "--git-dir"])
            .output()
            .await?;

        if !git_check.status.success() {
            return Err(anyhow::anyhow!(
                "Not a git repository. Initialize with 'git init' first."
            ));
        }

        // Get current branch name
        let branch_output = Command::new("git")
            .current_dir(&self.project_path)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .await?;

        if branch_output.status.success() {
            self.original_branch = Some(
                String::from_utf8_lossy(&branch_output.stdout)
                    .trim()
                    .to_string(),
            );
        }

        // Create worktree base directory if using worktrees
        if self.use_worktrees {
            std::fs::create_dir_all(&self.worktree_base)?;
        }

        Ok(())
    }

    /// Create an isolated workspace for a task
    pub async fn create_workspace(&mut self, task_id: &str) -> Result<PathBuf> {
        // Initialize if not done
        if self.original_branch.is_none() {
            self.init().await?;
        }

        let branch_name = format!("safe-coder/{}", task_id);

        if self.use_worktrees {
            self.create_worktree(task_id, &branch_name).await
        } else {
            self.create_branch(task_id, &branch_name).await
        }
    }

    /// Create a git worktree for isolation
    async fn create_worktree(&mut self, task_id: &str, branch_name: &str) -> Result<PathBuf> {
        let worktree_path = self.worktree_base.join(task_id);
        let worktree_path_str = worktree_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid UTF-8 in worktree path"))?;

        // Prune stale worktree entries (missing directories but still registered)
        let _ = Command::new("git")
            .current_dir(&self.project_path)
            .args(["worktree", "prune"])
            .output()
            .await;

        // Clean up any existing worktree and branch from previous runs
        if worktree_path.exists() {
            // Remove existing worktree
            let _ = Command::new("git")
                .current_dir(&self.project_path)
                .args(["worktree", "remove", worktree_path_str, "--force"])
                .output()
                .await;

            // Also try to remove the directory if worktree remove didn't clean it
            let _ = std::fs::remove_dir_all(&worktree_path);
        }

        // Delete existing branch if it exists
        let _ = Command::new("git")
            .current_dir(&self.project_path)
            .args(["branch", "-D", branch_name])
            .output()
            .await;

        // Create new branch from current HEAD
        let create_branch = Command::new("git")
            .current_dir(&self.project_path)
            .args(["branch", branch_name])
            .output()
            .await?;

        if !create_branch.status.success() {
            // Branch might already exist, try to continue
            let stderr = String::from_utf8_lossy(&create_branch.stderr);
            if !stderr.contains("already exists") {
                return Err(anyhow::anyhow!("Failed to create branch: {}", stderr));
            }
        }

        // Create worktree
        let create_worktree = Command::new("git")
            .current_dir(&self.project_path)
            .args(["worktree", "add", worktree_path_str, branch_name])
            .output()
            .await?;

        if !create_worktree.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to create worktree: {}",
                String::from_utf8_lossy(&create_worktree.stderr)
            ));
        }

        self.workspaces
            .insert(task_id.to_string(), worktree_path.clone());

        Ok(worktree_path)
    }

    /// Create a branch for isolation (simpler, uses main repo)
    async fn create_branch(&mut self, task_id: &str, branch_name: &str) -> Result<PathBuf> {
        // Create and checkout new branch
        let create_branch = Command::new("git")
            .current_dir(&self.project_path)
            .args(["checkout", "-b", branch_name])
            .output()
            .await?;

        if !create_branch.status.success() {
            // Try just checking out if it exists
            let checkout = Command::new("git")
                .current_dir(&self.project_path)
                .args(["checkout", branch_name])
                .output()
                .await?;

            if !checkout.status.success() {
                return Err(anyhow::anyhow!(
                    "Failed to create/checkout branch: {}",
                    String::from_utf8_lossy(&checkout.stderr)
                ));
            }
        }

        self.workspaces
            .insert(task_id.to_string(), self.project_path.clone());

        Ok(self.project_path.clone())
    }

    /// Merge a workspace back to the main branch
    pub async fn merge_workspace(&mut self, task_id: &str) -> Result<()> {
        let branch_name = format!("safe-coder/{}", task_id);

        if self.use_worktrees {
            self.merge_worktree(task_id, &branch_name).await
        } else {
            self.merge_branch(&branch_name).await
        }
    }

    /// Merge a worktree's changes back
    async fn merge_worktree(&mut self, task_id: &str, branch_name: &str) -> Result<()> {
        let _original_branch = self
            .original_branch
            .as_ref()
            .context("Original branch not known")?;

        // First, commit any changes in the worktree
        if let Some(worktree_path) = self.workspaces.get(task_id) {
            let _ = Command::new("git")
                .current_dir(worktree_path)
                .args(["add", "."])
                .output()
                .await?;

            let _ = Command::new("git")
                .current_dir(worktree_path)
                .args(["commit", "-m", &format!("Task {} completed", task_id)])
                .output()
                .await;
        }

        // Merge the branch into the original branch
        let merge = Command::new("git")
            .current_dir(&self.project_path)
            .args(["merge", branch_name, "--no-edit"])
            .output()
            .await?;

        if !merge.status.success() {
            let stderr = String::from_utf8_lossy(&merge.stderr);
            // Check if it's a conflict
            if stderr.contains("CONFLICT") {
                return Err(anyhow::anyhow!(
                    "Merge conflict when integrating task {}. Manual resolution needed.",
                    task_id
                ));
            }
            // Non-conflict error
            return Err(anyhow::anyhow!(
                "Failed to merge task {}: {}",
                task_id,
                stderr
            ));
        }

        Ok(())
    }

    /// Merge a branch back to original
    async fn merge_branch(&self, branch_name: &str) -> Result<()> {
        let original_branch = self
            .original_branch
            .as_ref()
            .context("Original branch not known")?;

        // Checkout original branch
        let checkout = Command::new("git")
            .current_dir(&self.project_path)
            .args(["checkout", original_branch])
            .output()
            .await?;

        if !checkout.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to checkout original branch: {}",
                String::from_utf8_lossy(&checkout.stderr)
            ));
        }

        // Merge the task branch
        let merge = Command::new("git")
            .current_dir(&self.project_path)
            .args(["merge", branch_name, "--no-edit"])
            .output()
            .await?;

        if !merge.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to merge branch: {}",
                String::from_utf8_lossy(&merge.stderr)
            ));
        }

        Ok(())
    }

    /// Cleanup a single workspace
    pub async fn cleanup_workspace(&mut self, task_id: &str) -> Result<()> {
        let branch_name = format!("safe-coder/{}", task_id);

        if self.use_worktrees {
            // Remove worktree
            if let Some(worktree_path) = self.workspaces.remove(task_id) {
                if let Some(path_str) = worktree_path.to_str() {
                    let _ = Command::new("git")
                        .current_dir(&self.project_path)
                        .args(["worktree", "remove", path_str, "--force"])
                        .output()
                        .await;
                }
            }
        }

        // Delete the branch
        let _ = Command::new("git")
            .current_dir(&self.project_path)
            .args(["branch", "-D", &branch_name])
            .output()
            .await;

        Ok(())
    }

    /// Cleanup all workspaces
    pub async fn cleanup_all(&mut self) -> Result<()> {
        let task_ids: Vec<String> = self.workspaces.keys().cloned().collect();

        for task_id in task_ids {
            self.cleanup_workspace(&task_id).await?;
        }

        // Remove the worktree base directory
        if self.worktree_base.exists() {
            let _ = std::fs::remove_dir_all(&self.worktree_base);
        }

        // Return to original branch
        if let Some(original) = &self.original_branch {
            let _ = Command::new("git")
                .current_dir(&self.project_path)
                .args(["checkout", original])
                .output()
                .await;
        }

        Ok(())
    }

    /// List all active workspaces
    pub fn list_workspaces(&self) -> Vec<(String, PathBuf)> {
        self.workspaces
            .iter()
            .map(|(id, path)| (id.clone(), path.clone()))
            .collect()
    }

    /// Get the workspace path for a task
    pub fn get_workspace(&self, task_id: &str) -> Option<&PathBuf> {
        self.workspaces.get(task_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_workspace_manager_creation() {
        let temp = tempdir().unwrap();
        let manager = WorkspaceManager::new(temp.path().to_path_buf(), true).unwrap();

        assert!(manager.workspaces.is_empty());
    }
}
