use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::mcp::McpConfig;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    pub llm: LlmConfig,
    #[serde(default)]
    pub git: GitConfig,
    #[serde(default)]
    pub orchestrator: OrchestratorConfig,
    #[serde(default)]
    pub tools: ToolConfig,
    #[serde(default)]
    pub lsp: LspConfigWrapper,
    #[serde(default)]
    pub cache: CacheConfig,
    #[serde(default)]
    pub mcp: McpConfig,
    #[serde(default)]
    pub checkpoint: CheckpointConfig,
    #[serde(default)]
    pub subagents: SubagentConfig,
    #[serde(default)]
    pub build: BuildConfig,
}

/// Configuration for subagent models
/// Allows different LLM providers/models for each subagent type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubagentConfig {
    /// Model for Code Analyzer subagent (read-only analysis)
    #[serde(default)]
    pub analyzer: Option<SubagentModelConfig>,
    /// Model for Tester subagent (test creation/execution)
    #[serde(default)]
    pub tester: Option<SubagentModelConfig>,
    /// Model for Refactorer subagent (code improvements)
    #[serde(default)]
    pub refactorer: Option<SubagentModelConfig>,
    /// Model for Documenter subagent (documentation)
    #[serde(default)]
    pub documenter: Option<SubagentModelConfig>,
    /// Model for Custom subagent
    #[serde(default)]
    pub custom: Option<SubagentModelConfig>,
}

impl Default for SubagentConfig {
    fn default() -> Self {
        Self {
            analyzer: None,
            tester: None,
            refactorer: None,
            documenter: None,
            custom: None,
        }
    }
}

/// Per-subagent model configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SubagentModelConfig {
    /// Provider to use (anthropic, openai, openrouter, ollama)
    pub provider: LlmProvider,
    /// Model name/ID
    pub model: String,
    /// Optional API key (falls back to main config or env var)
    #[serde(default)]
    pub api_key: Option<String>,
    /// Max tokens for this subagent
    #[serde(default = "default_subagent_max_tokens")]
    pub max_tokens: usize,
}

fn default_subagent_max_tokens() -> usize {
    4096
}

/// LSP configuration wrapper
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LspConfigWrapper {
    /// Whether LSP is enabled globally
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Per-language server configurations (overrides defaults)
    #[serde(default)]
    pub servers: std::collections::HashMap<String, LspServerConfigEntry>,
}

/// LSP server configuration entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LspServerConfigEntry {
    /// Whether this server is disabled
    #[serde(default)]
    pub disabled: bool,
    /// Command to start the server (overrides default)
    pub command: Option<String>,
    /// Arguments to pass to the command
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

impl Default for LspConfigWrapper {
    fn default() -> Self {
        Self {
            enabled: true,
            servers: std::collections::HashMap::new(),
        }
    }
}

/// Configuration for tool execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolConfig {
    /// Timeout for bash commands in seconds
    #[serde(default = "default_bash_timeout")]
    pub bash_timeout_secs: u64,
    /// Maximum output size in bytes
    #[serde(default = "default_max_output")]
    pub max_output_bytes: usize,
    /// Warn before executing dangerous commands
    #[serde(default = "default_true")]
    pub warn_dangerous_commands: bool,
    /// Regex patterns for dangerous commands to block
    #[serde(default = "default_dangerous_patterns")]
    pub dangerous_patterns: Vec<String>,
}

fn default_bash_timeout() -> u64 {
    120
}

fn default_max_output() -> usize {
    1_048_576 // 1 MB
}

fn default_dangerous_patterns() -> Vec<String> {
    vec![
        r"rm\s+(-[a-zA-Z]*)?-rf\s+[/~]".to_string(), // rm -rf / or ~
        r":\s*\(\s*\)\s*\{.*\}".to_string(),         // fork bomb :(){ :|:& };:
        r">\s*/dev/sd[a-z]".to_string(),             // overwrite disk
        r"mkfs\.".to_string(),                       // format filesystem
        r"dd\s+.*of=/dev/sd[a-z]".to_string(),       // dd to disk
        r"chmod\s+(-[a-zA-Z]*\s+)?777\s+/".to_string(), // chmod 777 /
        r"curl\s+.*\|\s*bash".to_string(),           // curl | bash (remote code exec)
        r"wget\s+.*\|\s*bash".to_string(),           // wget | bash
    ]
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            bash_timeout_secs: default_bash_timeout(),
            max_output_bytes: default_max_output(),
            warn_dangerous_commands: true,
            dangerous_patterns: default_dangerous_patterns(),
        }
    }
}

/// Configuration for build verification
/// Allows configuring build commands for different project types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildConfig {
    /// Whether automatic build verification is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Build commands by project type (detected by marker file)
    /// Key is the marker file (e.g., "Cargo.toml"), value is the build command
    #[serde(default = "default_build_commands")]
    pub commands: std::collections::HashMap<String, String>,
    /// Timeout for build commands in seconds
    #[serde(default = "default_build_timeout")]
    pub timeout_secs: u64,
    /// Maximum build output size in bytes before truncation
    #[serde(default = "default_build_max_output")]
    pub max_output_bytes: usize,
}

fn default_build_timeout() -> u64 {
    60
}

fn default_build_max_output() -> usize {
    8192 // 8KB - enough for error messages
}

fn default_build_commands() -> std::collections::HashMap<String, String> {
    let mut commands = std::collections::HashMap::new();
    // Rust
    commands.insert("Cargo.toml".to_string(), "cargo build 2>&1".to_string());
    // Node.js / TypeScript
    commands.insert(
        "tsconfig.json".to_string(),
        "npx tsc --noEmit 2>&1".to_string(),
    );
    commands.insert("package.json".to_string(), "npm run build 2>&1".to_string());
    // Go
    commands.insert("go.mod".to_string(), "go build ./... 2>&1".to_string());
    // Python
    commands.insert(
        "pyproject.toml".to_string(),
        "python -m compileall -q . 2>&1".to_string(),
    );
    commands.insert(
        "setup.py".to_string(),
        "python -m compileall -q . 2>&1".to_string(),
    );
    // C/C++ with CMake
    commands.insert(
        "CMakeLists.txt".to_string(),
        "cmake --build build 2>&1".to_string(),
    );
    // Java/Kotlin with Gradle
    commands.insert("build.gradle".to_string(), "gradle build 2>&1".to_string());
    commands.insert(
        "build.gradle.kts".to_string(),
        "gradle build 2>&1".to_string(),
    );
    // Java with Maven
    commands.insert("pom.xml".to_string(), "mvn compile 2>&1".to_string());
    commands
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            commands: default_build_commands(),
            timeout_secs: default_build_timeout(),
            max_output_bytes: default_build_max_output(),
        }
    }
}

impl BuildConfig {
    /// Get the build command for a project based on detected marker files
    pub fn get_build_command(&self, project_path: &std::path::Path) -> Option<String> {
        if !self.enabled {
            return None;
        }

        // Check markers in priority order (most specific first)
        let priority_order = [
            "Cargo.toml",       // Rust (most specific)
            "tsconfig.json",    // TypeScript
            "go.mod",           // Go
            "CMakeLists.txt",   // C/C++
            "build.gradle.kts", // Kotlin Gradle
            "build.gradle",     // Java Gradle
            "pom.xml",          // Maven
            "pyproject.toml",   // Python (modern)
            "setup.py",         // Python (legacy)
            "package.json",     // Node.js (least specific, check last)
        ];

        for marker in priority_order {
            if project_path.join(marker).exists() {
                if let Some(cmd) = self.commands.get(marker) {
                    return Some(cmd.clone());
                }
            }
        }

        None
    }

    /// Get a summary of the build command for display in prompts
    pub fn get_build_command_hint(&self, project_path: &std::path::Path) -> String {
        if let Some(cmd) = self.get_build_command(project_path) {
            format!("Build command: `{}`", cmd)
        } else {
            "No build command detected. Run appropriate build/compile command for your project."
                .to_string()
        }
    }
}

/// Configuration for LLM response caching
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CacheConfig {
    /// Whether caching is enabled globally
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Use provider-native caching (Anthropic cache_control, OpenAI automatic)
    #[serde(default = "default_true")]
    pub provider_native: bool,
    /// Use application-level response caching
    #[serde(default = "default_true")]
    pub application_cache: bool,
    /// Maximum number of cached responses
    #[serde(default = "default_cache_max_entries")]
    pub max_entries: usize,
    /// Time-to-live for cached responses in minutes
    #[serde(default = "default_cache_ttl_minutes")]
    pub ttl_minutes: u64,
}

fn default_cache_max_entries() -> usize {
    100
}

fn default_cache_ttl_minutes() -> u64 {
    30
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            provider_native: true,
            application_cache: true,
            max_entries: default_cache_max_entries(),
            ttl_minutes: default_cache_ttl_minutes(),
        }
    }
}

impl CacheConfig {
    /// Convert to the llm::cached::CacheConfig type
    pub fn to_llm_cache_config(&self) -> crate::llm::cached::CacheConfig {
        crate::llm::cached::CacheConfig {
            enabled: self.enabled,
            provider_native: self.provider_native,
            application_cache: self.application_cache,
            ttl: std::time::Duration::from_secs(self.ttl_minutes * 60),
            max_entries: self.max_entries,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitConfig {
    /// Enable automatic git commits after tool execution
    #[serde(default = "default_true")]
    pub auto_commit: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub api_key: Option<String>,
    pub model: String,
    pub max_tokens: usize,
    /// Base URL for API (optional, for Ollama or custom endpoints)
    #[serde(default)]
    pub base_url: Option<String>,
    /// [BETA/DANGEROUS] Enable Claude Code OAuth compatibility mode.
    /// This injects a system prompt to make OAuth tokens work with the API.
    /// May violate Anthropic's Terms of Service. Use at your own risk.
    #[serde(default)]
    pub claude_code_oauth_compat: bool,
}

/// Configuration for the CLI orchestrator
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrchestratorConfig {
    /// Path to Claude Code CLI executable
    #[serde(default = "default_claude_cli")]
    pub claude_cli_path: String,
    /// Path to Gemini CLI executable
    #[serde(default = "default_gemini_cli")]
    pub gemini_cli_path: String,
    /// Path to Safe-Coder CLI executable (defaults to current exe)
    #[serde(default = "default_safe_coder_cli")]
    pub safe_coder_cli_path: String,
    /// Path to GitHub CLI (for Copilot)
    #[serde(default = "default_gh_cli")]
    pub gh_cli_path: String,
    /// Maximum number of concurrent workers
    #[serde(default = "default_max_workers")]
    pub max_workers: usize,
    /// Default worker to use: "claude", "gemini", "safe-coder", or "github-copilot"
    #[serde(default = "default_worker")]
    pub default_worker: String,
    /// Worker distribution strategy: "single", "round-robin", "task-based", or "load-balanced"
    #[serde(default = "default_worker_strategy")]
    pub worker_strategy: String,
    /// List of enabled workers for multi-worker strategies
    #[serde(default = "default_enabled_workers")]
    pub enabled_workers: Vec<String>,
    /// Use git worktrees for task isolation
    #[serde(default = "default_true")]
    pub use_worktrees: bool,
    /// Throttle limits for worker types
    #[serde(default)]
    pub throttle_limits: ThrottleLimitsConfig,
}

/// Throttle limits configuration for different worker types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ThrottleLimitsConfig {
    /// Maximum concurrent Claude Code workers
    #[serde(default = "default_claude_max")]
    pub claude_max_concurrent: usize,
    /// Maximum concurrent Gemini CLI workers
    #[serde(default = "default_gemini_max")]
    pub gemini_max_concurrent: usize,
    /// Maximum concurrent Safe-Coder workers
    #[serde(default = "default_safe_coder_max")]
    pub safe_coder_max_concurrent: usize,
    /// Maximum concurrent GitHub Copilot workers
    #[serde(default = "default_copilot_max")]
    pub copilot_max_concurrent: usize,
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

fn default_safe_coder_cli() -> String {
    // Default to current executable path or "safe-coder" in PATH
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "safe-coder".to_string())
}

fn default_gh_cli() -> String {
    "gh".to_string()
}

fn default_max_workers() -> usize {
    3
}

fn default_worker() -> String {
    "claude".to_string()
}

fn default_worker_strategy() -> String {
    "single".to_string()
}

fn default_enabled_workers() -> Vec<String> {
    vec!["claude".to_string()]
}

fn default_claude_max() -> usize {
    2
}

fn default_gemini_max() -> usize {
    2
}

fn default_safe_coder_max() -> usize {
    2
}

fn default_copilot_max() -> usize {
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
            safe_coder_max_concurrent: default_safe_coder_max(),
            copilot_max_concurrent: default_copilot_max(),
            start_delay_ms: default_start_delay(),
        }
    }
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            claude_cli_path: default_claude_cli(),
            gemini_cli_path: default_gemini_cli(),
            safe_coder_cli_path: default_safe_coder_cli(),
            gh_cli_path: default_gh_cli(),
            max_workers: default_max_workers(),
            default_worker: default_worker(),
            worker_strategy: default_worker_strategy(),
            enabled_workers: default_enabled_workers(),
            use_worktrees: true,
            throttle_limits: ThrottleLimitsConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    Anthropic,
    OpenAI,
    Ollama,
    #[serde(rename = "github-copilot")]
    GitHubCopilot,
    #[serde(rename = "openrouter")]
    OpenRouter,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content =
            std::fs::read_to_string(&config_path).context("Failed to read config file")?;

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
        let config_dir = dirs::config_dir().context("Could not determine config directory")?;
        Ok(config_dir.join("safe-coder").join("config.toml"))
    }

    pub fn token_path(provider: &LlmProvider) -> Result<PathBuf> {
        let config_dir = dirs::config_dir().context("Could not determine config directory")?;
        let token_file = match provider {
            LlmProvider::Anthropic => "anthropic_token.json",
            LlmProvider::GitHubCopilot => "github_copilot_token.json",
            _ => {
                return Err(anyhow::anyhow!(
                    "Provider does not support device flow auth"
                ))
            }
        };
        Ok(config_dir.join("safe-coder").join(token_file))
    }

    /// Get the stored token for the current provider (if any)
    pub fn get_stored_token(&self) -> Option<crate::auth::StoredToken> {
        match Self::token_path(&self.llm.provider) {
            Ok(token_path) => {
                tracing::debug!("Looking for token at: {:?}", token_path);
                if token_path.exists() {
                    use crate::auth::StoredToken;
                    match StoredToken::load(&token_path) {
                        Ok(stored_token) => {
                            tracing::debug!("Successfully loaded token from {:?}", token_path);
                            return Some(stored_token);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to load token from {:?}: {}", token_path, e);
                        }
                    }
                } else {
                    tracing::debug!("Token file does not exist: {:?}", token_path);
                }
            }
            Err(e) => {
                tracing::debug!("Could not get token path for provider: {}", e);
            }
        }
        None
    }

    /// Get the effective API key/token for the current provider
    /// Checks for stored tokens first, then falls back to configured API key
    pub fn get_auth_token(&self) -> Result<String> {
        tracing::debug!("get_auth_token called for provider: {:?}", self.llm.provider);

        // First check if there's a stored token for this provider
        if let Some(stored_token) = self.get_stored_token() {
            if !stored_token.is_expired() {
                tracing::debug!("Using stored token (not expired)");
                return Ok(stored_token.get_access_token().to_string());
            } else {
                tracing::debug!("Stored token is expired");
            }
        } else {
            tracing::debug!("No stored token found for provider");
        }

        // Fall back to configured API key
        tracing::debug!("Falling back to configured API key");
        self.llm
            .api_key
            .clone()
            .context("No API key or valid token found")
    }

    /// Get the model configuration for a specific subagent kind
    /// Returns None if no specific config is set (falls back to main LLM)
    pub fn get_subagent_model(&self, kind: &str) -> Option<&SubagentModelConfig> {
        match kind {
            "analyzer" | "code_analyzer" => self.subagents.analyzer.as_ref(),
            "tester" => self.subagents.tester.as_ref(),
            "refactorer" => self.subagents.refactorer.as_ref(),
            "documenter" => self.subagents.documenter.as_ref(),
            "custom" => self.subagents.custom.as_ref(),
            _ => None,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        // Try to detect provider from environment variables
        let (provider, api_key, model) = if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
            (
                LlmProvider::Anthropic,
                Some(key),
                "claude-sonnet-4-20250514".to_string(),
            )
        } else if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            (LlmProvider::OpenAI, Some(key), "gpt-4o".to_string())
        } else if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
            // OpenRouter with Claude as default model
            (
                LlmProvider::OpenRouter,
                Some(key),
                "anthropic/claude-3.5-sonnet".to_string(),
            )
        } else if let Ok(key) = std::env::var("GITHUB_COPILOT_TOKEN") {
            (LlmProvider::GitHubCopilot, Some(key), "gpt-4".to_string())
        } else {
            // Default to Anthropic even without key
            (
                LlmProvider::Anthropic,
                None,
                "claude-sonnet-4-20250514".to_string(),
            )
        };

        Self {
            llm: LlmConfig {
                provider,
                api_key,
                model,
                max_tokens: 8192,
                base_url: None,
                claude_code_oauth_compat: false,
            },
            git: GitConfig::default(),
            orchestrator: OrchestratorConfig::default(),
            tools: ToolConfig::default(),
            lsp: LspConfigWrapper::default(),
            cache: CacheConfig::default(),
            mcp: McpConfig::default(),
            checkpoint: CheckpointConfig::default(),
            subagents: SubagentConfig::default(),
            build: BuildConfig::default(),
        }
    }
}

impl SubagentModelConfig {
    /// Get the API key for this subagent config, falling back to environment variables
    pub fn get_api_key(&self) -> Option<String> {
        // First check explicit config
        if let Some(ref key) = self.api_key {
            return Some(key.clone());
        }

        // Fall back to environment variables based on provider
        match self.provider {
            LlmProvider::Anthropic => std::env::var("ANTHROPIC_API_KEY").ok(),
            LlmProvider::OpenAI => std::env::var("OPENAI_API_KEY").ok(),
            LlmProvider::OpenRouter => std::env::var("OPENROUTER_API_KEY").ok(),
            LlmProvider::GitHubCopilot => std::env::var("GITHUB_COPILOT_TOKEN").ok(),
            LlmProvider::Ollama => None, // Ollama doesn't need API key
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

/// Configuration for directory-based checkpoints (git-agnostic)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckpointConfig {
    /// Enable directory-based checkpoints before each task
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Maximum number of checkpoints to keep (oldest deleted first)
    #[serde(default = "default_max_checkpoints")]
    pub max_checkpoints: usize,
    /// Custom storage path (default: .safe-coder-checkpoints in project dir)
    #[serde(default)]
    pub storage_path: Option<String>,
    /// Patterns to ignore when creating checkpoints (gitignore-style)
    #[serde(default = "default_ignore_patterns_checkpoint")]
    pub ignore_patterns: Vec<String>,
}

fn default_max_checkpoints() -> usize {
    10
}

fn default_ignore_patterns_checkpoint() -> Vec<String> {
    vec![
        "node_modules/".to_string(),
        "target/".to_string(),
        ".git/".to_string(),
        "__pycache__/".to_string(),
        ".venv/".to_string(),
        "venv/".to_string(),
        ".safe-coder-checkpoints/".to_string(),
        "*.pyc".to_string(),
        ".DS_Store".to_string(),
        "Thumbs.db".to_string(),
    ]
}

impl Default for CheckpointConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_checkpoints: default_max_checkpoints(),
            storage_path: None,
            ignore_patterns: default_ignore_patterns_checkpoint(),
        }
    }
}
