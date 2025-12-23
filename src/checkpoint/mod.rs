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

    #[tokio::test]
    async fn test_checkpoint_restore() {
        let temp_dir = TempDir::new().unwrap();
        let sandbox = temp_dir.path().to_path_buf();
        let mut manager = CheckpointManager::new(sandbox.clone());

        // Create a test file
        let test_file = sandbox.join("test.txt");
        fs::write(&test_file, "original content").await.unwrap();

        // Create checkpoint with a label
        manager.create_checkpoint("test checkpoint").await.unwrap();

        // Modify file
        fs::write(&test_file, "modified content").await.unwrap();

        // Restore
        manager.restore_file(&test_file).await.unwrap();

        // Check content
        let content = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(content, "original content");
    }
}
