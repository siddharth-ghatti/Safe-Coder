use anyhow::{Context, Result};
use std::path::Path;
use tokio::process::Command;

pub struct GitManager {
    repo_path: std::path::PathBuf,
}

impl GitManager {
    pub fn new(repo_path: std::path::PathBuf) -> Self {
        Self { repo_path }
    }

    /// Initialize git repo in the VM if not already present
    pub async fn init_if_needed(&self) -> Result<()> {
        let git_dir = self.repo_path.join(".git");

        if !git_dir.exists() {
            tracing::info!("Initializing git repository in VM");

            // Initialize repo
            Command::new("git")
                .arg("init")
                .current_dir(&self.repo_path)
                .output()
                .await
                .context("Failed to initialize git repo")?;

            // Configure git
            Command::new("git")
                .args(["config", "user.name", "Safe Coder VM"])
                .current_dir(&self.repo_path)
                .output()
                .await?;

            Command::new("git")
                .args(["config", "user.email", "vm@safe-coder.dev"])
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
                .args(["commit", "-m", "Initial snapshot - Safe Coder VM"])
                .current_dir(&self.repo_path)
                .output()
                .await?;

            tracing::info!("Git repository initialized with initial snapshot");
        } else {
            tracing::info!("Git repository already exists in VM");
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
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("nothing to commit") {
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
            self.files.iter()
                .map(|f| format!("  - {}", f))
                .collect::<Vec<_>>()
                .join("\n")
        )
    }
}
