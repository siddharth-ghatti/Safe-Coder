use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::process::Command;

/// Git-based checkpoint system for undo/restore
pub struct CheckpointManager {
    repo_path: PathBuf,
}

impl CheckpointManager {
    /// Create new checkpoint manager
    pub fn new(repo_path: PathBuf) -> Self {
        Self { repo_path }
    }

    /// Create checkpoint using git stash
    pub async fn create_checkpoint(&mut self, label: &str) -> Result<()> {
        Command::new("git")
            .args(["stash", "push", "-m", &format!("Checkpoint: {}", label)])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to create git checkpoint")?;

        tracing::debug!("Created git checkpoint: {}", label);

        Ok(())
    }

    /// Restore file using git checkout
    pub async fn restore_file(&mut self, file_path: &Path) -> Result<()> {
        let path_str = file_path.to_str()
            .context("Invalid file path")?;

        Command::new("git")
            .args(["checkout", "HEAD", "--", path_str])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to restore file")?;

        tracing::info!("Restored file: {}", file_path.display());

        Ok(())
    }

    /// Restore all files to HEAD (undo all changes)
    pub async fn restore_all(&mut self) -> Result<Vec<PathBuf>> {
        // Get list of changed files first
        let output = Command::new("git")
            .args(["diff", "--name-only", "HEAD"])
            .current_dir(&self.repo_path)
            .output()
            .await?;

        let changed_files: Vec<PathBuf> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(PathBuf::from)
            .collect();

        // Reset to HEAD
        Command::new("git")
            .args(["reset", "--hard", "HEAD"])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to restore all files")?;

        tracing::info!("Restored all files to HEAD");

        Ok(changed_files)
    }

    /// Rollback to previous commit
    pub async fn rollback_commit(&mut self) -> Result<()> {
        Command::new("git")
            .args(["reset", "--hard", "HEAD~1"])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to rollback commit")?;

        tracing::warn!("Rolled back to previous commit");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;
    use tokio::process::Command;

    #[tokio::test]
    async fn test_checkpoint_restore() {
        let temp_dir = TempDir::new().unwrap();
        let sandbox = temp_dir.path().to_path_buf();

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(&sandbox)
            .output()
            .await
            .unwrap();

        // Configure git user for commits
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&sandbox)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(&sandbox)
            .output()
            .await
            .unwrap();

        // Create a test file
        let test_file = sandbox.join("test.txt");
        fs::write(&test_file, "original content").await.unwrap();

        // Add and commit the file (needed for git checkout to work)
        Command::new("git")
            .args(["add", "test.txt"])
            .current_dir(&sandbox)
            .output()
            .await
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&sandbox)
            .output()
            .await
            .unwrap();

        let mut manager = CheckpointManager::new(sandbox.clone());

        // Modify file
        fs::write(&test_file, "modified content").await.unwrap();

        // Restore using git checkout HEAD
        manager.restore_file(&test_file).await.unwrap();

        // Check content is restored to original
        let content = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(content, "original content");
    }
}
