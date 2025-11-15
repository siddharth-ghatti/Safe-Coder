use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use uuid::Uuid;

use crate::config::DockerConfig;
use crate::git::{ChangeSummary, GitManager};
use super::IsolationBackend;

pub struct DockerBackend {
    config: DockerConfig,
    instance: Option<DockerInstance>,
}

struct DockerInstance {
    id: Uuid,
    container_id: String,
    project_path: PathBuf,
    shared_dir: PathBuf,
    git_manager: GitManager,
}

impl DockerBackend {
    pub fn new(config: DockerConfig) -> Self {
        Self {
            config,
            instance: None,
        }
    }

    async fn ensure_image(&self) -> Result<()> {
        if !self.config.auto_pull {
            return Ok(());
        }

        // Check if image exists
        let output = Command::new("docker")
            .args(["image", "inspect", &self.config.image])
            .output()
            .await?;

        if !output.status.success() {
            tracing::info!("🐳 Pulling Docker image: {}", self.config.image);
            let output = Command::new("docker")
                .args(["pull", &self.config.image])
                .output()
                .await?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Failed to pull Docker image: {}", stderr);
            }
            tracing::info!("✓ Docker image pulled");
        }

        Ok(())
    }

    fn copy_dir_all(&self, src: &Path, dst: &Path) -> Result<()> {
        if !dst.exists() {
            std::fs::create_dir_all(dst)?;
        }

        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if ty.is_dir() {
                self.copy_dir_all(&src_path, &dst_path)?;
            } else {
                std::fs::copy(&src_path, &dst_path)?;
            }
        }

        Ok(())
    }

    fn copy_dir_all_excluding(&self, src: &Path, dst: &Path, excludes: &[&str]) -> Result<()> {
        if !dst.exists() {
            std::fs::create_dir_all(dst)?;
        }

        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let file_name = entry.file_name();
            let file_name_str = file_name.to_string_lossy();

            // Skip excluded files/directories
            if excludes.contains(&file_name_str.as_ref()) {
                tracing::debug!("Skipping excluded: {}", file_name_str);
                continue;
            }

            let ty = entry.file_type()?;
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());

            if ty.is_dir() {
                self.copy_dir_all_excluding(&src_path, &dst_path, excludes)?;
            } else {
                std::fs::copy(&src_path, &dst_path)?;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl IsolationBackend for DockerBackend {
    async fn start(&mut self, project_path: PathBuf) -> Result<PathBuf> {
        if self.instance.is_some() {
            return Ok(self.instance.as_ref().unwrap().shared_dir.clone());
        }

        // Ensure Docker image is available
        self.ensure_image().await?;

        let id = Uuid::new_v4();
        let container_name = format!("safe-coder-{}", id);

        // Create a shared directory for the project
        let shared_dir = std::env::temp_dir().join(format!("safe-coder-{}", id));
        std::fs::create_dir_all(&shared_dir)?;

        // 🔒 ISOLATION: Copy project files to Docker sandbox
        tracing::info!("🐳 Creating isolated copy of project in Docker container");
        if project_path.exists() {
            self.copy_dir_all(&project_path, &shared_dir)?;
            tracing::info!("✓ Project copied to container sandbox: {}", shared_dir.display());
        } else {
            std::fs::create_dir_all(&shared_dir)?;
            tracing::info!("✓ Created empty container sandbox: {}", shared_dir.display());
        }

        // 🔒 Initialize git tracking in container sandbox
        let git_manager = GitManager::new(shared_dir.clone());
        git_manager.init_if_needed().await?;
        tracing::info!("✓ Git tracking initialized in container");

        // Create Docker container
        // Note: We're using the shared_dir as a volume mount for simplicity
        // In production, you might want to copy files into the container instead
        let output = Command::new("docker")
            .args([
                "create",
                "--name", &container_name,
                "--cpus", &self.config.cpus.to_string(),
                "--memory", &format!("{}m", self.config.memory_mb),
                "--network", "none",  // No network access for security
                "--volume", &format!("{}:/workspace", shared_dir.display()),
                "--workdir", "/workspace",
                &self.config.image,
                "sleep", "infinity",  // Keep container running
            ])
            .output()
            .await
            .context("Failed to create Docker container")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to create Docker container: {}", stderr);
        }

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        tracing::info!("Created Docker container: {}", container_id);

        // Start the container
        let output = Command::new("docker")
            .args(["start", &container_id])
            .output()
            .await
            .context("Failed to start Docker container")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Failed to start Docker container: {}", stderr);
        }

        tracing::info!("Started Docker container with ID: {}", container_id);

        self.instance = Some(DockerInstance {
            id,
            container_id,
            project_path: project_path.clone(),
            shared_dir: shared_dir.clone(),
            git_manager,
        });

        tracing::info!("🐳 Docker container isolation active - agent confined to sandbox");

        Ok(shared_dir)
    }

    async fn stop(&mut self) -> Result<()> {
        if let Some(_instance) = &self.instance {
            // Sync files back to project (excluding .git)
            tracing::info!("🔒 Syncing container changes back to host...");
            self.sync_back(false).await?;
        }

        if let Some(instance) = self.instance.take() {
            // Stop and remove the container
            let _ = Command::new("docker")
                .args(["stop", &instance.container_id])
                .output()
                .await;

            let _ = Command::new("docker")
                .args(["rm", &instance.container_id])
                .output()
                .await;

            // Cleanup shared directory
            let _ = std::fs::remove_dir_all(&instance.shared_dir);

            tracing::info!("✓ Stopped Docker container {}", instance.id);
        }

        Ok(())
    }

    fn get_sandbox_dir(&self) -> Option<&Path> {
        self.instance.as_ref().map(|i| i.shared_dir.as_path())
    }

    fn get_project_path(&self) -> Option<&Path> {
        self.instance.as_ref().map(|i| i.project_path.as_path())
    }

    fn get_git_manager(&self) -> Option<&GitManager> {
        self.instance.as_ref().map(|i| &i.git_manager)
    }

    async fn commit_changes(&self, message: &str) -> Result<()> {
        if let Some(instance) = &self.instance {
            instance.git_manager.auto_commit(message).await?;
        }
        Ok(())
    }

    async fn get_changes(&self) -> Result<ChangeSummary> {
        if let Some(instance) = &self.instance {
            instance.git_manager.get_change_summary().await
        } else {
            anyhow::bail!("Container not running")
        }
    }

    async fn sync_back(&self, force: bool) -> Result<()> {
        if let Some(instance) = &self.instance {
            let changes = instance.git_manager.get_change_summary().await?;

            if !force && changes.has_changes() {
                tracing::warn!("🔒 Changes detected in container:");
                tracing::warn!("{}", changes.summary_text());
                tracing::warn!("Syncing to host...");
            }

            // Copy files back, excluding .git directory
            self.copy_dir_all_excluding(&instance.shared_dir, &instance.project_path, &[".git"])?;
            tracing::info!("✓ Changes synced to host: {}", instance.project_path.display());
        }
        Ok(())
    }

    fn backend_name(&self) -> &str {
        "Docker"
    }
}

impl Drop for DockerBackend {
    fn drop(&mut self) {
        if let Some(instance) = self.instance.take() {
            // Best effort cleanup
            let _ = std::process::Command::new("docker")
                .args(["stop", &instance.container_id])
                .output();

            let _ = std::process::Command::new("docker")
                .args(["rm", &instance.container_id])
                .output();

            let _ = std::fs::remove_dir_all(&instance.shared_dir);
        }
    }
}
