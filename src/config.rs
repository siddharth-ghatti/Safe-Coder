use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
    #[serde(default)]
    pub git: GitConfig,
    #[serde(default)]
    pub orchestrator: OrchestratorConfig,
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

/// Configuration for the CLI orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// Path to Claude Code CLI executable
    #[serde(default = "default_claude_cli")]
    pub claude_cli_path: String,
    /// Path to Gemini CLI executable
    #[serde(default = "default_gemini_cli")]
    pub gemini_cli_path: String,
    /// Maximum number of concurrent workers
    #[serde(default = "default_max_workers")]
    pub max_workers: usize,
    /// Default worker to use: "claude" or "gemini"
    #[serde(default = "default_worker")]
    pub default_worker: String,
    /// Use git worktrees for task isolation
    #[serde(default = "default_true")]
    pub use_worktrees: bool,
    /// Throttle limits for worker types
    #[serde(default)]
    pub throttle_limits: ThrottleLimitsConfig,
}

/// Throttle limits configuration for different worker types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThrottleLimitsConfig {
    /// Maximum concurrent Claude Code workers
    #[serde(default = "default_claude_max")]
    pub claude_max_concurrent: usize,
    /// Maximum concurrent Gemini CLI workers
    #[serde(default = "default_gemini_max")]
    pub gemini_max_concurrent: usize,
    /// Delay between starting workers of the same type (milliseconds)
    #[serde(default = "default_start_delay")]
    pub start_delay_ms: u64,
}

fn default_claude_cli() -> String {
    "claude".to_string()
}

fn default_gemini_cli() -> String {
    "gemini".to_string()
}

fn default_max_workers() -> usize {
    3
}

fn default_worker() -> String {
    "claude".to_string()
}

fn default_claude_max() -> usize {
    2
}

fn default_gemini_max() -> usize {
    2
}

fn default_start_delay() -> u64 {
    100
}

impl Default for ThrottleLimitsConfig {
    fn default() -> Self {
        Self {
            claude_max_concurrent: default_claude_max(),
            gemini_max_concurrent: default_gemini_max(),
            start_delay_ms: default_start_delay(),
        }
    }
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            claude_cli_path: default_claude_cli(),
            gemini_cli_path: default_gemini_cli(),
            max_workers: default_max_workers(),
            default_worker: default_worker(),
            use_worktrees: true,
            throttle_limits: ThrottleLimitsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Anthropic,
    OpenAI,
    Ollama,
    #[serde(rename = "github-copilot")]
    GitHubCopilot,
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

    pub fn token_path(provider: &LlmProvider) -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?;
        let token_file = match provider {
            LlmProvider::Anthropic => "anthropic_token.json",
            LlmProvider::GitHubCopilot => "github_copilot_token.json",
            _ => return Err(anyhow::anyhow!("Provider does not support device flow auth")),
        };
        Ok(config_dir.join("safe-coder").join(token_file))
    }

    /// Get the effective API key/token for the current provider
    /// Checks for stored tokens first, then falls back to configured API key
    pub fn get_auth_token(&self) -> Result<String> {
        // First check if there's a stored token for this provider
        if let Ok(token_path) = Self::token_path(&self.llm.provider) {
            if token_path.exists() {
                use crate::auth::StoredToken;
                if let Ok(stored_token) = StoredToken::load(&token_path) {
                    if !stored_token.is_expired() {
                        return Ok(stored_token.access_token);
                    }
                }
            }
        }

        // Fall back to configured API key
        self.llm.api_key
            .clone()
            .context("No API key or valid token found")
    }
}

impl Default for Config {
    fn default() -> Self {
        // Try to detect provider from environment variables
        let (provider, api_key, model) = if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            (LlmProvider::Anthropic, Some(key), "claude-sonnet-4-20250514".to_string())
        } else if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            (LlmProvider::OpenAI, Some(key), "gpt-4o".to_string())
        } else if let Ok(key) = std::env::var("GITHUB_COPILOT_TOKEN") {
            (LlmProvider::GitHubCopilot, Some(key), "gpt-4".to_string())
        } else {
            // Default to Anthropic even without key
            (LlmProvider::Anthropic, None, "claude-sonnet-4-20250514".to_string())
        };

        Self {
            llm: LlmConfig {
                provider,
                api_key,
                model,
                max_tokens: 8192,
                base_url: None,
            },
            git: GitConfig::default(),
            orchestrator: OrchestratorConfig::default(),
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
