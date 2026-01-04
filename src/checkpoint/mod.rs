use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use glob::Pattern;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;

use crate::config::CheckpointConfig;

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
        let path_str = file_path.to_str().context("Invalid file path")?;

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

// ============================================================================
// Directory-based Checkpoint System (Git-Agnostic)
// ============================================================================

/// Metadata for a single checkpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Unique identifier for this checkpoint
    pub id: String,
    /// When the checkpoint was created
    pub timestamp: DateTime<Utc>,
    /// Human-readable label (task description)
    pub label: String,
    /// Number of files in this checkpoint
    pub files_count: usize,
    /// Total size in bytes
    pub total_bytes: u64,
}

/// Metadata file containing all checkpoints
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CheckpointMetadata {
    checkpoints: Vec<Checkpoint>,
}

/// Git-agnostic directory-based checkpoint manager
/// Creates point-in-time copies of the working directory
pub struct DirectoryCheckpointManager {
    project_path: PathBuf,
    checkpoint_dir: PathBuf,
    config: CheckpointConfig,
    ignore_patterns: Vec<Pattern>,
}

impl DirectoryCheckpointManager {
    /// Create a new directory checkpoint manager
    pub fn new(project_path: PathBuf, config: CheckpointConfig) -> Result<Self> {
        // Determine checkpoint storage location
        let checkpoint_dir = if let Some(ref custom_path) = config.storage_path {
            PathBuf::from(custom_path)
        } else {
            project_path.join(".safe-coder-checkpoints")
        };

        // Compile ignore patterns
        let ignore_patterns: Vec<Pattern> = config
            .ignore_patterns
            .iter()
            .filter_map(|p| Pattern::new(p).ok())
            .collect();

        Ok(Self {
            project_path,
            checkpoint_dir,
            config,
            ignore_patterns,
        })
    }

    /// Check if the project is a git repository
    fn is_git_repo(&self) -> bool {
        self.project_path.join(".git").exists()
    }

    /// Ensure checkpoint directory is in .gitignore (if this is a git project)
    async fn ensure_gitignore(&self) -> Result<()> {
        if !self.is_git_repo() {
            return Ok(());
        }

        let gitignore_path = self.project_path.join(".gitignore");
        let checkpoint_entry = ".safe-coder-checkpoints/";

        // Read existing .gitignore or create empty content
        let existing_content = if gitignore_path.exists() {
            tokio::fs::read_to_string(&gitignore_path)
                .await
                .unwrap_or_default()
        } else {
            String::new()
        };

        // Check if checkpoint dir is already ignored
        let already_ignored = existing_content.lines().any(|line| {
            let trimmed = line.trim();
            trimmed == checkpoint_entry
                || trimmed == ".safe-coder-checkpoints"
                || trimmed == "/.safe-coder-checkpoints/"
                || trimmed == "/.safe-coder-checkpoints"
        });

        if already_ignored {
            return Ok(());
        }

        // Add checkpoint directory to .gitignore
        let new_content = if existing_content.is_empty() {
            format!(
                "# Safe-Coder checkpoints (auto-generated)\n{}\n",
                checkpoint_entry
            )
        } else if existing_content.ends_with('\n') {
            format!(
                "{}\n# Safe-Coder checkpoints (auto-generated)\n{}\n",
                existing_content, checkpoint_entry
            )
        } else {
            format!(
                "{}\n\n# Safe-Coder checkpoints (auto-generated)\n{}\n",
                existing_content, checkpoint_entry
            )
        };

        tokio::fs::write(&gitignore_path, new_content)
            .await
            .context("Failed to update .gitignore")?;

        tracing::info!("Added {} to .gitignore", checkpoint_entry);

        Ok(())
    }

    /// Check if checkpoints are enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Create a new checkpoint before a task
    pub async fn create_checkpoint(&mut self, label: &str) -> Result<String> {
        if !self.config.enabled {
            return Ok(String::new());
        }

        // Ensure checkpoint directory is in .gitignore (for git projects)
        if let Err(e) = self.ensure_gitignore().await {
            tracing::warn!("Failed to update .gitignore: {}", e);
        }

        // Generate checkpoint ID
        let id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let timestamp = Utc::now();

        // Create checkpoint directory structure
        let checkpoint_path = self.checkpoint_dir.join("checkpoints").join(&id);
        let files_path = checkpoint_path.join("files");
        tokio::fs::create_dir_all(&files_path)
            .await
            .context("Failed to create checkpoint directory")?;

        // Copy files
        let (files_count, total_bytes) = self.copy_project_files(&files_path).await?;

        // Create checkpoint entry
        let checkpoint = Checkpoint {
            id: id.clone(),
            timestamp,
            label: label.chars().take(100).collect(),
            files_count,
            total_bytes,
        };

        // Save manifest for this checkpoint
        let manifest_path = checkpoint_path.join("manifest.json");
        let manifest_content = serde_json::to_string_pretty(&checkpoint)?;
        tokio::fs::write(&manifest_path, manifest_content)
            .await
            .context("Failed to write checkpoint manifest")?;

        // Update global metadata
        self.add_checkpoint_to_metadata(checkpoint).await?;

        // Cleanup old checkpoints
        self.cleanup_old_checkpoints().await?;

        tracing::info!(
            "Created checkpoint {} ({} files, {} bytes): {}",
            id,
            files_count,
            total_bytes,
            label
        );

        Ok(id)
    }

    /// Copy project files to checkpoint directory
    async fn copy_project_files(&self, dest: &Path) -> Result<(usize, u64)> {
        let mut files_count = 0usize;
        let mut total_bytes = 0u64;

        // Use WalkBuilder which respects .gitignore automatically
        let walker = WalkBuilder::new(&self.project_path)
            .hidden(false) // Include hidden files
            .git_ignore(true) // Respect .gitignore
            .git_global(false)
            .git_exclude(true)
            .follow_links(false)
            .build();

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();

            // Skip directories (we'll create them as needed)
            if path.is_dir() {
                continue;
            }

            // Check against our ignore patterns
            if self.should_ignore(path) {
                continue;
            }

            // Get relative path
            let relative = match path.strip_prefix(&self.project_path) {
                Ok(r) => r,
                Err(_) => continue,
            };

            // Skip the checkpoint directory itself
            if relative.starts_with(".safe-coder-checkpoints") {
                continue;
            }

            // Create destination path
            let dest_path = dest.join(relative);

            // Create parent directories
            if let Some(parent) = dest_path.parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }

            // Copy file
            match tokio::fs::copy(path, &dest_path).await {
                Ok(bytes) => {
                    files_count += 1;
                    total_bytes += bytes;
                }
                Err(e) => {
                    tracing::debug!("Skipping file {}: {}", path.display(), e);
                }
            }
        }

        Ok((files_count, total_bytes))
    }

    /// Check if a path should be ignored
    fn should_ignore(&self, path: &Path) -> bool {
        let relative = match path.strip_prefix(&self.project_path) {
            Ok(r) => r,
            Err(_) => return false,
        };

        let path_str = relative.to_string_lossy();

        // Check against ignore patterns
        for pattern in &self.ignore_patterns {
            if pattern.matches(&path_str) {
                return true;
            }
            // Also check if any component matches (for directory patterns)
            for component in relative.components() {
                let comp_str = component.as_os_str().to_string_lossy();
                if pattern.matches(&comp_str) || pattern.matches(&format!("{}/", comp_str)) {
                    return true;
                }
            }
        }

        false
    }

    /// Load metadata from disk
    async fn load_metadata(&self) -> Result<CheckpointMetadata> {
        let metadata_path = self.checkpoint_dir.join("metadata.json");

        if !metadata_path.exists() {
            return Ok(CheckpointMetadata::default());
        }

        let content = tokio::fs::read_to_string(&metadata_path)
            .await
            .context("Failed to read checkpoint metadata")?;

        serde_json::from_str(&content).context("Failed to parse checkpoint metadata")
    }

    /// Save metadata to disk
    async fn save_metadata(&self, metadata: &CheckpointMetadata) -> Result<()> {
        tokio::fs::create_dir_all(&self.checkpoint_dir).await.ok();

        let metadata_path = self.checkpoint_dir.join("metadata.json");
        let content = serde_json::to_string_pretty(metadata)?;

        tokio::fs::write(&metadata_path, content)
            .await
            .context("Failed to write checkpoint metadata")
    }

    /// Add a checkpoint to metadata
    async fn add_checkpoint_to_metadata(&self, checkpoint: Checkpoint) -> Result<()> {
        let mut metadata = self.load_metadata().await?;
        metadata.checkpoints.push(checkpoint);
        self.save_metadata(&metadata).await
    }

    /// List all checkpoints
    pub async fn list_checkpoints(&self) -> Result<Vec<Checkpoint>> {
        let metadata = self.load_metadata().await?;
        Ok(metadata.checkpoints)
    }

    /// Restore to a specific checkpoint
    pub async fn restore_checkpoint(&self, checkpoint_id: &str) -> Result<()> {
        let checkpoint_path = self
            .checkpoint_dir
            .join("checkpoints")
            .join(checkpoint_id)
            .join("files");

        if !checkpoint_path.exists() {
            anyhow::bail!("Checkpoint '{}' not found", checkpoint_id);
        }

        // Copy files back from checkpoint
        self.restore_files_from(&checkpoint_path).await?;

        tracing::info!("Restored checkpoint: {}", checkpoint_id);

        Ok(())
    }

    /// Restore the most recent checkpoint
    pub async fn restore_latest(&self) -> Result<()> {
        let metadata = self.load_metadata().await?;

        let latest = metadata
            .checkpoints
            .last()
            .context("No checkpoints available")?;

        self.restore_checkpoint(&latest.id).await
    }

    /// Restore files from a checkpoint directory
    async fn restore_files_from(&self, source: &Path) -> Result<()> {
        let walker = WalkBuilder::new(source)
            .hidden(false)
            .git_ignore(false) // Don't respect gitignore when restoring
            .follow_links(false)
            .build();

        for entry in walker {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let path = entry.path();

            if path.is_dir() {
                continue;
            }

            // Get relative path
            let relative = match path.strip_prefix(source) {
                Ok(r) => r,
                Err(_) => continue,
            };

            // Create destination path in project
            let dest_path = self.project_path.join(relative);

            // Create parent directories
            if let Some(parent) = dest_path.parent() {
                tokio::fs::create_dir_all(parent).await.ok();
            }

            // Copy file back
            tokio::fs::copy(path, &dest_path)
                .await
                .context(format!("Failed to restore file: {}", relative.display()))?;
        }

        Ok(())
    }

    /// Delete a specific checkpoint
    pub async fn delete_checkpoint(&mut self, checkpoint_id: &str) -> Result<()> {
        // Remove from metadata
        let mut metadata = self.load_metadata().await?;
        metadata.checkpoints.retain(|c| c.id != checkpoint_id);
        self.save_metadata(&metadata).await?;

        // Remove checkpoint directory
        let checkpoint_path = self.checkpoint_dir.join("checkpoints").join(checkpoint_id);
        if checkpoint_path.exists() {
            tokio::fs::remove_dir_all(&checkpoint_path)
                .await
                .context("Failed to delete checkpoint directory")?;
        }

        tracing::info!("Deleted checkpoint: {}", checkpoint_id);

        Ok(())
    }

    /// Remove old checkpoints beyond max_checkpoints limit
    async fn cleanup_old_checkpoints(&mut self) -> Result<()> {
        let mut metadata = self.load_metadata().await?;

        while metadata.checkpoints.len() > self.config.max_checkpoints {
            if let Some(oldest) = metadata.checkpoints.first() {
                let id = oldest.id.clone();

                // Remove checkpoint directory
                let checkpoint_path = self.checkpoint_dir.join("checkpoints").join(&id);
                if checkpoint_path.exists() {
                    tokio::fs::remove_dir_all(&checkpoint_path).await.ok();
                }

                metadata.checkpoints.remove(0);
                tracing::debug!("Cleaned up old checkpoint: {}", id);
            } else {
                break;
            }
        }

        self.save_metadata(&metadata).await?;

        Ok(())
    }

    /// Get checkpoint storage path
    pub fn checkpoint_dir(&self) -> &Path {
        &self.checkpoint_dir
    }

    /// Format checkpoint list for display
    pub fn format_checkpoint_list(checkpoints: &[Checkpoint]) -> String {
        if checkpoints.is_empty() {
            return "No checkpoints found.".to_string();
        }

        let mut output = String::new();
        output.push_str("📦 Checkpoints\n");
        output.push_str("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n\n");

        for checkpoint in checkpoints.iter().rev() {
            let size = if checkpoint.total_bytes > 1_000_000 {
                format!("{:.1} MB", checkpoint.total_bytes as f64 / 1_000_000.0)
            } else if checkpoint.total_bytes > 1_000 {
                format!("{:.1} KB", checkpoint.total_bytes as f64 / 1_000.0)
            } else {
                format!("{} bytes", checkpoint.total_bytes)
            };

            output.push_str(&format!(
                "📍 {} ({})\n   Created: {}\n   Files: {} | Size: {}\n   Label: {}\n\n",
                checkpoint.id,
                checkpoint.timestamp.format("%Y-%m-%d %H:%M:%S"),
                checkpoint.timestamp.format("%b %d, %H:%M"),
                checkpoint.files_count,
                size,
                checkpoint.label
            ));
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CheckpointConfig;
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

    #[tokio::test]
    async fn test_directory_checkpoint_creates_gitignore_entry() {
        let temp_dir = TempDir::new().unwrap();
        let sandbox = temp_dir.path().to_path_buf();

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(&sandbox)
            .output()
            .await
            .unwrap();

        // Create a test file
        let test_file = sandbox.join("test.txt");
        fs::write(&test_file, "test content").await.unwrap();

        // Create checkpoint manager
        let config = CheckpointConfig::default();
        let mut manager = DirectoryCheckpointManager::new(sandbox.clone(), config).unwrap();

        // Create a checkpoint (this should add to .gitignore)
        manager.create_checkpoint("test checkpoint").await.unwrap();

        // Verify .gitignore was created and contains the checkpoint dir
        let gitignore_path = sandbox.join(".gitignore");
        assert!(gitignore_path.exists(), ".gitignore should be created");

        let gitignore_content = fs::read_to_string(&gitignore_path).await.unwrap();
        assert!(
            gitignore_content.contains(".safe-coder-checkpoints"),
            ".gitignore should contain checkpoint directory"
        );
    }

    #[tokio::test]
    async fn test_directory_checkpoint_preserves_existing_gitignore() {
        let temp_dir = TempDir::new().unwrap();
        let sandbox = temp_dir.path().to_path_buf();

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(&sandbox)
            .output()
            .await
            .unwrap();

        // Create existing .gitignore with some content
        let gitignore_path = sandbox.join(".gitignore");
        fs::write(&gitignore_path, "node_modules/\n*.log\n")
            .await
            .unwrap();

        // Create a test file
        let test_file = sandbox.join("test.txt");
        fs::write(&test_file, "test content").await.unwrap();

        // Create checkpoint manager and checkpoint
        let config = CheckpointConfig::default();
        let mut manager = DirectoryCheckpointManager::new(sandbox.clone(), config).unwrap();
        manager.create_checkpoint("test checkpoint").await.unwrap();

        // Verify existing content is preserved
        let gitignore_content = fs::read_to_string(&gitignore_path).await.unwrap();
        assert!(
            gitignore_content.contains("node_modules/"),
            "Existing entries should be preserved"
        );
        assert!(
            gitignore_content.contains("*.log"),
            "Existing entries should be preserved"
        );
        assert!(
            gitignore_content.contains(".safe-coder-checkpoints"),
            "Checkpoint dir should be added"
        );
    }

    #[tokio::test]
    async fn test_directory_checkpoint_does_not_duplicate_gitignore_entry() {
        let temp_dir = TempDir::new().unwrap();
        let sandbox = temp_dir.path().to_path_buf();

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(&sandbox)
            .output()
            .await
            .unwrap();

        // Create .gitignore that already has the entry
        let gitignore_path = sandbox.join(".gitignore");
        fs::write(&gitignore_path, ".safe-coder-checkpoints/\n")
            .await
            .unwrap();

        // Create a test file
        let test_file = sandbox.join("test.txt");
        fs::write(&test_file, "test content").await.unwrap();

        // Create checkpoint manager and checkpoint
        let config = CheckpointConfig::default();
        let mut manager = DirectoryCheckpointManager::new(sandbox.clone(), config).unwrap();
        manager.create_checkpoint("test checkpoint").await.unwrap();

        // Verify no duplicate entries
        let gitignore_content = fs::read_to_string(&gitignore_path).await.unwrap();
        let count = gitignore_content.matches(".safe-coder-checkpoints").count();
        assert_eq!(count, 1, "Should not duplicate gitignore entry");
    }

    #[tokio::test]
    async fn test_directory_checkpoint_no_gitignore_for_non_git_project() {
        let temp_dir = TempDir::new().unwrap();
        let sandbox = temp_dir.path().to_path_buf();

        // Do NOT initialize git repo

        // Create a test file
        let test_file = sandbox.join("test.txt");
        fs::write(&test_file, "test content").await.unwrap();

        // Create checkpoint manager and checkpoint
        let config = CheckpointConfig::default();
        let mut manager = DirectoryCheckpointManager::new(sandbox.clone(), config).unwrap();
        manager.create_checkpoint("test checkpoint").await.unwrap();

        // Verify .gitignore was NOT created (not a git project)
        let gitignore_path = sandbox.join(".gitignore");
        assert!(
            !gitignore_path.exists(),
            ".gitignore should not be created for non-git projects"
        );
    }

    #[tokio::test]
    async fn test_directory_checkpoint_create_and_restore() {
        let temp_dir = TempDir::new().unwrap();
        let sandbox = temp_dir.path().to_path_buf();

        // Create test files
        let test_file = sandbox.join("test.txt");
        fs::write(&test_file, "original content").await.unwrap();

        // Create checkpoint manager
        let config = CheckpointConfig::default();
        let mut manager = DirectoryCheckpointManager::new(sandbox.clone(), config).unwrap();

        // Create a checkpoint
        let checkpoint_id = manager.create_checkpoint("before changes").await.unwrap();
        assert!(!checkpoint_id.is_empty());

        // Modify the file
        fs::write(&test_file, "modified content").await.unwrap();
        let modified = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(modified, "modified content");

        // Restore the checkpoint
        manager.restore_checkpoint(&checkpoint_id).await.unwrap();

        // Verify content is restored
        let restored = fs::read_to_string(&test_file).await.unwrap();
        assert_eq!(restored, "original content");
    }
}
