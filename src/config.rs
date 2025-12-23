use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub llm: LlmConfig,
    #[serde(default)]
    pub git: GitConfig,
    #[serde(default)]
    pub tools: ToolConfig,
}

/// Configuration for tool execution limits and behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// Default timeout for bash commands in seconds (default: 120)
    #[serde(default = "default_bash_timeout")]
    pub bash_timeout_secs: u64,

    /// Maximum output size in bytes before truncation (default: 1MB)
    #[serde(default = "default_max_output_bytes")]
    pub max_output_bytes: usize,

    /// Enable dangerous command warnings (default: true)
    #[serde(default = "default_true")]
    pub warn_dangerous_commands: bool,

    /// List of command patterns to warn about (regexes)
    #[serde(default = "default_dangerous_patterns")]
    pub dangerous_patterns: Vec<String>,
}

fn default_bash_timeout() -> u64 {
    120
}

fn default_max_output_bytes() -> usize {
    1_048_576 // 1MB
}

fn default_dangerous_patterns() -> Vec<String> {
    vec![
        r"rm\s+-rf\s+/".to_string(),
        r"rm\s+-rf\s+~".to_string(),
        r":()\s*\{\s*:\|\:&\s*\}".to_string(), // Fork bomb
        r"dd\s+if=.*of=/dev/".to_string(),
        r"mkfs\.".to_string(),
        r">\s*/dev/sd".to_string(),
        r"chmod\s+-R\s+777\s+/".to_string(),
    ]
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            bash_timeout_secs: default_bash_timeout(),
            max_output_bytes: default_max_output_bytes(),
            warn_dangerous_commands: true,
            dangerous_patterns: default_dangerous_patterns(),
        }
    }
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

    /// Get the stored token for the current provider (if any)
    pub fn get_stored_token(&self) -> Option<crate::auth::StoredToken> {
        if let Ok(token_path) = Self::token_path(&self.llm.provider) {
            if token_path.exists() {
                use crate::auth::StoredToken;
                if let Ok(stored_token) = StoredToken::load(&token_path) {
                    return Some(stored_token);
                }
            }
        }
        None
    }

    /// Get the effective API key/token for the current provider
    /// Checks for stored tokens first, then falls back to configured API key
    pub fn get_auth_token(&self) -> Result<String> {
        // First check if there's a stored token for this provider
        if let Some(stored_token) = self.get_stored_token() {
            if !stored_token.is_expired() {
                return Ok(stored_token.get_access_token().to_string());
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
            tools: ToolConfig::default(),
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
