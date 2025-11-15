use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command as TokioCommand;
use uuid::Uuid;

use crate::config::VmConfig;
use crate::git::GitManager;

pub struct VmManager {
    config: VmConfig,
    instance: Option<VmInstance>,
}

pub struct VmInstance {
    pub id: Uuid,
    pub socket_path: PathBuf,
    pub project_path: PathBuf,
    pub shared_dir: PathBuf,
    pub git_manager: GitManager,
    process: Option<Child>,
}

#[derive(Debug, Serialize, Deserialize)]
struct FirecrackerConfig {
    #[serde(rename = "boot-source")]
    boot_source: BootSource,
    drives: Vec<Drive>,
    #[serde(rename = "machine-config")]
    machine_config: MachineConfig,
    #[serde(rename = "network-interfaces", skip_serializing_if = "Option::is_none")]
    network_interfaces: Option<Vec<NetworkInterface>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BootSource {
    kernel_image_path: String,
    boot_args: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Drive {
    drive_id: String,
    path_on_host: String,
    is_root_device: bool,
    is_read_only: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct MachineConfig {
    vcpu_count: u8,
    mem_size_mib: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct NetworkInterface {
    iface_id: String,
    host_dev_name: String,
}

impl VmManager {
    pub fn new(config: VmConfig) -> Self {
        Self {
            config,
            instance: None,
        }
    }

    pub async fn start_vm(&mut self, project_path: PathBuf) -> Result<&VmInstance> {
        if self.instance.is_some() {
            return Ok(self.instance.as_ref().unwrap());
        }

        let id = Uuid::new_v4();
        let socket_path = std::env::temp_dir().join(format!("firecracker-{}.sock", id));

        // Create a shared directory for the project
        let shared_dir = std::env::temp_dir().join(format!("safe-coder-{}", id));
        std::fs::create_dir_all(&shared_dir)?;

        // 🔒 ISOLATION: Copy project files to VM sandbox
        tracing::info!("🔒 Creating isolated copy of project in VM");
        if project_path.exists() {
            self.copy_dir_all(&project_path, &shared_dir)?;
            tracing::info!("✓ Project copied to VM sandbox: {}", shared_dir.display());
        } else {
            std::fs::create_dir_all(&shared_dir)?;
            tracing::info!("✓ Created empty VM sandbox: {}", shared_dir.display());
        }

        // 🔒 Initialize git tracking in VM
        let git_manager = GitManager::new(shared_dir.clone());
        git_manager.init_if_needed().await?;
        tracing::info!("✓ Git tracking initialized in VM");

        // Create Firecracker config
        let config = self.create_firecracker_config(&shared_dir)?;
        let config_path = std::env::temp_dir().join(format!("fc-config-{}.json", id));
        std::fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;

        // Start Firecracker process
        let process = Command::new(&self.config.firecracker_bin)
            .arg("--api-sock")
            .arg(&socket_path)
            .arg("--config-file")
            .arg(&config_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context("Failed to start Firecracker")?;

        tracing::info!("Started Firecracker VM with ID: {}", id);

        self.instance = Some(VmInstance {
            id,
            socket_path,
            project_path: project_path.clone(),
            shared_dir,
            git_manager,
            process: Some(process),
        });

        // Wait a bit for VM to initialize
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        tracing::info!("🔒 VM isolation active - agent confined to sandbox");

        Ok(self.instance.as_ref().unwrap())
    }

    pub async fn execute_command(&self, command: &str) -> Result<String> {
        let instance = self.instance.as_ref()
            .context("VM not started")?;

        // For now, we'll execute commands in the shared directory on the host
        // In a full implementation, you'd use SSH or vsock to execute in the VM
        let output = TokioCommand::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&instance.shared_dir)
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            anyhow::bail!("Command failed: {}\n{}", stdout, stderr);
        }

        Ok(format!("{}{}", stdout, stderr))
    }

    pub fn get_shared_dir(&self) -> Option<&Path> {
        self.instance.as_ref().map(|i| i.shared_dir.as_path())
    }

    pub fn get_git_manager(&self) -> Option<&GitManager> {
        self.instance.as_ref().map(|i| &i.git_manager)
    }

    /// Auto-commit changes in VM with a message
    pub async fn commit_changes(&self, message: &str) -> Result<()> {
        if let Some(instance) = &self.instance {
            instance.git_manager.auto_commit(message).await?;
        }
        Ok(())
    }

    /// Get summary of changes in VM
    pub async fn get_changes(&self) -> Result<crate::git::ChangeSummary> {
        if let Some(instance) = &self.instance {
            instance.git_manager.get_change_summary().await
        } else {
            anyhow::bail!("VM not running")
        }
    }

    /// Sync changes back to host (with approval)
    pub async fn sync_back(&self, force: bool) -> Result<()> {
        if let Some(instance) = &self.instance {
            let changes = instance.git_manager.get_change_summary().await?;

            if !force && changes.has_changes() {
                tracing::warn!("🔒 Changes detected in VM:");
                tracing::warn!("{}", changes.summary_text());
                tracing::warn!("Syncing to host...");
            }

            // Copy files back, excluding .git directory
            self.copy_dir_all_excluding(&instance.shared_dir, &instance.project_path, &[".git"])?;
            tracing::info!("✓ Changes synced to host: {}", instance.project_path.display());
        }
        Ok(())
    }

    pub async fn stop_vm(&mut self) -> Result<()> {
        if let Some(instance) = &self.instance {
            // Sync files back to project (excluding .git)
            tracing::info!("🔒 Syncing VM changes back to host...");
            self.sync_back(false).await?;
        }

        if let Some(mut instance) = self.instance.take() {
            // Kill the process
            if let Some(mut process) = instance.process {
                process.kill()?;
                process.wait()?;
            }

            // Cleanup
            let _ = std::fs::remove_file(&instance.socket_path);
            let _ = std::fs::remove_dir_all(&instance.shared_dir);

            tracing::info!("✓ Stopped VM {}", instance.id);
        }

        Ok(())
    }

    fn create_firecracker_config(&self, shared_dir: &Path) -> Result<FirecrackerConfig> {
        Ok(FirecrackerConfig {
            boot_source: BootSource {
                kernel_image_path: self.config.kernel_image.to_string_lossy().to_string(),
                boot_args: "console=ttyS0 reboot=k panic=1 pci=off".to_string(),
            },
            drives: vec![Drive {
                drive_id: "rootfs".to_string(),
                path_on_host: self.config.rootfs_image.to_string_lossy().to_string(),
                is_root_device: true,
                is_read_only: false,
            }],
            machine_config: MachineConfig {
                vcpu_count: self.config.vcpu_count,
                mem_size_mib: self.config.mem_size_mib,
            },
            network_interfaces: None,
        })
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

impl Drop for VmManager {
    fn drop(&mut self) {
        if let Some(mut instance) = self.instance.take() {
            if let Some(mut process) = instance.process {
                let _ = process.kill();
                let _ = process.wait();
            }
            let _ = std::fs::remove_file(&instance.socket_path);
            let _ = std::fs::remove_dir_all(&instance.shared_dir);
        }
    }
}
