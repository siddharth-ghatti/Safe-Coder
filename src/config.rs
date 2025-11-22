use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
    #[serde(default)]
    pub git: GitConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    /// Enable automatic git commits after tool execution
    #[serde(default = "default_true")]
    pub auto_commit: bool,
}

fn default_true() -> bool {
    true
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
            git: GitConfig::default(),
        }
    }
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            auto_commit: true, // Enabled by default
        }
    }
}
