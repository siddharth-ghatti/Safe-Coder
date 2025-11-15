use anyhow::Result;
use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::git::{ChangeSummary, GitManager};

pub mod firecracker;
pub mod docker;

pub use firecracker::FirecrackerBackend;
pub use docker::DockerBackend;

/// Trait for isolation backends (Firecracker, Docker, etc.)
#[async_trait]
pub trait IsolationBackend: Send + Sync {
    /// Start the isolated environment and return the sandbox directory
    async fn start(&mut self, project_path: PathBuf) -> Result<PathBuf>;

    /// Stop the isolated environment and cleanup
    async fn stop(&mut self) -> Result<()>;

    /// Get the sandbox directory path (where agent operates)
    fn get_sandbox_dir(&self) -> Option<&Path>;

    /// Get the original project path
    fn get_project_path(&self) -> Option<&Path>;

    /// Get the git manager for change tracking
    fn get_git_manager(&self) -> Option<&GitManager>;

    /// Commit changes in the sandbox
    async fn commit_changes(&self, message: &str) -> Result<()>;

    /// Get summary of changes
    async fn get_changes(&self) -> Result<ChangeSummary>;

    /// Sync changes back to host
    async fn sync_back(&self, force: bool) -> Result<()>;

    /// Get backend name for display
    fn backend_name(&self) -> &str;
}

/// Auto-detect and create the appropriate isolation backend
pub async fn create_backend(config: &crate::config::Config) -> Result<Box<dyn IsolationBackend>> {
    use crate::config::IsolationBackend as BackendType;

    let backend_type = &config.isolation.backend;

    match backend_type {
        BackendType::Firecracker => {
            // Verify we're on Linux
            if !cfg!(target_os = "linux") {
                anyhow::bail!(
                    "Firecracker requires Linux. Current OS: {}. Use 'docker' backend instead.",
                    std::env::consts::OS
                );
            }
            tracing::info!("🔥 Using Firecracker microVM isolation (maximum security)");
            Ok(Box::new(FirecrackerBackend::new(config.vm.clone())))
        }
        BackendType::Docker => {
            tracing::info!("🐳 Using Docker container isolation");
            Ok(Box::new(DockerBackend::new(config.docker.clone())))
        }
        BackendType::Auto => {
            // Auto-detect based on platform
            if cfg!(target_os = "linux") {
                tracing::info!("🔥 Auto-selected Firecracker (Linux detected)");
                Ok(Box::new(FirecrackerBackend::new(config.vm.clone())))
            } else {
                tracing::info!("🐳 Auto-selected Docker ({} detected)", std::env::consts::OS);
                Ok(Box::new(DockerBackend::new(config.docker.clone())))
            }
        }
    }
}
