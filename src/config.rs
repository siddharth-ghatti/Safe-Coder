use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
    pub vm: VmConfig,
    #[serde(default)]
    pub isolation: IsolationConfig,
    #[serde(default)]
    pub docker: DockerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub api_key: Option<String>,
    pub model: String,
    pub max_tokens: usize,
    /// Base URL for API (optional, for Ollama or custom endpoints)
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Anthropic,
    OpenAI,
    Ollama,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmConfig {
    pub firecracker_bin: PathBuf,
    pub kernel_image: PathBuf,
    pub rootfs_image: PathBuf,
    pub vcpu_count: u8,
    pub mem_size_mib: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IsolationConfig {
    /// Backend to use: "auto", "firecracker", or "docker"
    /// - auto: Firecracker on Linux, Docker on other platforms (default)
    /// - firecracker: Force Firecracker (requires Linux)
    /// - docker: Force Docker (works on all platforms)
    pub backend: IsolationBackend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IsolationBackend {
    Auto,
    Firecracker,
    Docker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerConfig {
    /// Docker image to use for isolation
    pub image: String,
    /// CPU limit (number of CPUs)
    pub cpus: f32,
    /// Memory limit in MB
    pub memory_mb: usize,
    /// Whether to pull the image if not present
    pub auto_pull: bool,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&config_path)
            .context("Failed to read config file")?;

        toml::from_str(&content).context("Failed to parse config file")
    }

    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;

        Ok(())
    }

    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?;
        Ok(config_dir.join("safe-coder").join("config.toml"))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            llm: LlmConfig {
                provider: LlmProvider::Anthropic,
                api_key: std::env::var("ANTHROPIC_API_KEY").ok(),
                model: "claude-sonnet-4-20250514".to_string(),
                max_tokens: 8192,
                base_url: None,
            },
            vm: VmConfig {
                firecracker_bin: PathBuf::from("/usr/local/bin/firecracker"),
                kernel_image: PathBuf::from("/var/lib/safe-coder/vmlinux"),
                rootfs_image: PathBuf::from("/var/lib/safe-coder/rootfs.ext4"),
                vcpu_count: 2,
                mem_size_mib: 512,
            },
            isolation: IsolationConfig::default(),
            docker: DockerConfig::default(),
        }
    }
}

impl Default for IsolationConfig {
    fn default() -> Self {
        Self {
            backend: IsolationBackend::Auto,
        }
    }
}

impl Default for DockerConfig {
    fn default() -> Self {
        Self {
            image: "ubuntu:22.04".to_string(),
            cpus: 2.0,
            memory_mb: 512,
            auto_pull: true,
        }
    }
}
