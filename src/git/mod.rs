use anyhow::{Context, Result};

use tokio::process::Command;

pub struct GitManager {
    repo_path: std::path::PathBuf,
    /// Stack of commit hashes for redo functionality
    redo_stack: Vec<String>,
}

impl GitManager {
    pub fn new(repo_path: std::path::PathBuf) -> Self {
        Self {
            repo_path,
            redo_stack: Vec::new(),
        }
    }

    /// Check if this is a git repository
    pub fn is_git_repo(&self) -> bool {
        self.repo_path.join(".git").exists()
    }

    /// Get current HEAD commit hash
    pub async fn get_head_commit(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to get HEAD commit")?;

        if !output.status.success() {
            anyhow::bail!("Not a git repository or no commits yet");
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Undo the last change by resetting to HEAD~1
    /// Returns the files that were restored and saves current HEAD for redo
    pub async fn undo(&mut self) -> Result<UndoResult> {
        if !self.is_git_repo() {
            anyhow::bail!("Not a git repository. Use /checkpoint restore instead.");
        }

        // Get current HEAD before undo (for redo)
        let current_head = self.get_head_commit().await?;

        // Get list of files that will be affected
        let output = Command::new("git")
            .args(["diff", "--name-only", "HEAD~1", "HEAD"])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to get changed files")?;

        let files_changed: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        if files_changed.is_empty() {
            // Check if there's even a previous commit
            let log_output = Command::new("git")
                .args(["log", "--oneline", "-2"])
                .current_dir(&self.repo_path)
                .output()
                .await?;

            let log_str = String::from_utf8_lossy(&log_output.stdout);
            let log_lines: Vec<_> = log_str.lines().collect();

            if log_lines.len() < 2 {
                anyhow::bail!("No previous commit to undo to");
            }
        }

        // Reset to previous commit
        let reset_output = Command::new("git")
            .args(["reset", "--hard", "HEAD~1"])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to reset to previous commit")?;

        if !reset_output.status.success() {
            anyhow::bail!(
                "Failed to undo: {}",
                String::from_utf8_lossy(&reset_output.stderr)
            );
        }

        // Save the undone commit for redo
        self.redo_stack.push(current_head.clone());

        tracing::info!("Undo: reset to HEAD~1, saved {} for redo", current_head);

        Ok(UndoResult {
            files_restored: files_changed,
            commit_undone: current_head,
        })
    }

    /// Redo a previously undone change
    pub async fn redo(&mut self) -> Result<RedoResult> {
        if !self.is_git_repo() {
            anyhow::bail!("Not a git repository");
        }

        let commit_to_restore = self
            .redo_stack
            .pop()
            .ok_or_else(|| anyhow::anyhow!("Nothing to redo"))?;

        // Get files that will change
        let output = Command::new("git")
            .args(["diff", "--name-only", "HEAD", &commit_to_restore])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to get changed files")?;

        let files_changed: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        // Reset to the saved commit
        let reset_output = Command::new("git")
            .args(["reset", "--hard", &commit_to_restore])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to reset to saved commit")?;

        if !reset_output.status.success() {
            // Put it back on the stack since we failed
            self.redo_stack.push(commit_to_restore);
            anyhow::bail!(
                "Failed to redo: {}",
                String::from_utf8_lossy(&reset_output.stderr)
            );
        }

        tracing::info!("Redo: restored to commit {}", commit_to_restore);

        Ok(RedoResult {
            files_restored: files_changed,
            commit_restored: commit_to_restore,
        })
    }

    /// Check if redo is available
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clear redo stack (called when new changes are made)
    pub fn clear_redo_stack(&mut self) {
        self.redo_stack.clear();
    }

    /// Initialize git repo if not already present
    pub async fn init_if_needed(&self) -> Result<()> {
        let git_dir = self.repo_path.join(".git");

        if !git_dir.exists() {
            tracing::info!("Initializing git repository for Safe Coder");

            // Initialize repo
            Command::new("git")
                .arg("init")
                .current_dir(&self.repo_path)
                .output()
                .await
                .context("Failed to initialize git repo")?;

            // Configure git
            Command::new("git")
                .args(["config", "user.name", "Safe Coder"])
                .current_dir(&self.repo_path)
                .output()
                .await?;

            Command::new("git")
                .args(["config", "user.email", "ai@safe-coder.dev"])
                .current_dir(&self.repo_path)
                .output()
                .await?;

            // Initial commit
            Command::new("git")
                .args(["add", "."])
                .current_dir(&self.repo_path)
                .output()
                .await?;

            Command::new("git")
                .args([
                    "commit",
                    "-m",
                    "Initial snapshot - Safe Coder session start",
                ])
                .current_dir(&self.repo_path)
                .output()
                .await?;

            tracing::info!("Git repository initialized with initial snapshot");
        } else {
            tracing::info!("Git repository already exists");
        }

        Ok(())
    }

    /// Get current git status
    pub async fn status(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["status", "--short"])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to get git status")?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Get git diff
    pub async fn diff(&self) -> Result<String> {
        let output = Command::new("git")
            .args(["diff"])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to get git diff")?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Auto-commit changes made by the agent
    pub async fn auto_commit(&self, message: &str) -> Result<()> {
        // Add all changes
        Command::new("git")
            .args(["add", "."])
            .current_dir(&self.repo_path)
            .output()
            .await?;

        // Commit
        let output = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(&self.repo_path)
            .output()
            .await?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            // Check both stdout and stderr for "nothing to commit" message
            if stdout.contains("nothing to commit") || stderr.contains("nothing to commit") {
                tracing::debug!("No changes to commit");
                return Ok(());
            }
            anyhow::bail!("Git commit failed: {}", stderr);
        }

        tracing::info!("Auto-committed: {}", message);
        Ok(())
    }

    /// Get commit log
    pub async fn log(&self, count: usize) -> Result<String> {
        let output = Command::new("git")
            .args(["log", &format!("-{}", count), "--oneline"])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to get git log")?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Create a snapshot commit before major operations
    pub async fn snapshot(&self, label: &str) -> Result<()> {
        self.auto_commit(&format!("🔒 Snapshot: {}", label)).await
    }

    /// Get list of changed files
    pub async fn changed_files(&self) -> Result<Vec<String>> {
        let output = Command::new("git")
            .args(["diff", "--name-only", "HEAD"])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to get changed files")?;

        let files = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        Ok(files)
    }

    /// Rollback to previous commit (safety feature)
    pub async fn rollback(&self) -> Result<()> {
        tracing::warn!("Rolling back to previous commit");

        Command::new("git")
            .args(["reset", "--hard", "HEAD~1"])
            .current_dir(&self.repo_path)
            .output()
            .await
            .context("Failed to rollback")?;

        Ok(())
    }

    /// Get summary of changes for approval
    pub async fn get_change_summary(&self) -> Result<ChangeSummary> {
        let status = self.status().await?;
        let files = self.changed_files().await?;
        let diff = self.diff().await?;

        Ok(ChangeSummary {
            status,
            files,
            diff,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ChangeSummary {
    pub status: String,
    pub files: Vec<String>,
    pub diff: String,
}

impl ChangeSummary {
    pub fn has_changes(&self) -> bool {
        !self.files.is_empty()
    }

    pub fn summary_text(&self) -> String {
        if self.files.is_empty() {
            return "No changes".to_string();
        }

        format!(
            "Changed {} file(s):\n{}",
            self.files.len(),
            self.files
                .iter()
                .map(|f| format!("  - {}", f))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}

/// Result of an undo operation
#[derive(Debug, Clone)]
pub struct UndoResult {
    pub files_restored: Vec<String>,
    pub commit_undone: String,
}

impl UndoResult {
    pub fn format(&self) -> String {
        let mut output = String::new();
        output.push_str("↩️  Undo successful\n");

        if self.files_restored.is_empty() {
            output.push_str("No files were changed.\n");
        } else {
            output.push_str(&format!(
                "Restored {} file(s):\n",
                self.files_restored.len()
            ));
            for file in &self.files_restored {
                output.push_str(&format!("  • {}\n", file));
            }
        }

        output.push_str(&format!(
            "\nUndone commit: {}",
            &self.commit_undone[..8.min(self.commit_undone.len())]
        ));
        output.push_str("\nUse /redo to restore these changes.");

        output
    }
}

/// Result of a redo operation
#[derive(Debug, Clone)]
pub struct RedoResult {
    pub files_restored: Vec<String>,
    pub commit_restored: String,
}

impl RedoResult {
    pub fn format(&self) -> String {
        let mut output = String::new();
        output.push_str("↪️  Redo successful\n");

        if self.files_restored.is_empty() {
            output.push_str("No files were changed.\n");
        } else {
            output.push_str(&format!(
                "Restored {} file(s):\n",
                self.files_restored.len()
            ));
            for file in &self.files_restored {
                output.push_str(&format!("  • {}\n", file));
            }
        }

        output.push_str(&format!(
            "\nRestored to commit: {}",
            &self.commit_restored[..8.min(self.commit_restored.len())]
        ));

        output
    }
}
