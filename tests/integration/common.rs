use anyhow::Result;
use assert_fs::prelude::*;
use assert_fs::TempDir;
use safe_coder::config::{Config, LlmConfig, LlmProvider, GitConfig, OrchestratorConfig, ToolConfig, LspConfigWrapper, CacheConfig, CheckpointConfig, SubagentConfig};
use safe_coder::mcp::McpConfig;
use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::Command;

/// Test utilities for integration tests
pub struct TestEnvironment {
    pub temp_dir: TempDir,
    pub project_path: PathBuf,
    pub config_dir: TempDir,
}

impl TestEnvironment {
    /// Create a new test environment with a temporary project directory
    pub fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let config_dir = TempDir::new()?;
        let project_path = temp_dir.path().to_path_buf();

        // Set the config directory for tests
        env::set_var("XDG_CONFIG_HOME", config_dir.path());

        Ok(Self {
            temp_dir,
            project_path,
            config_dir,
        })
    }

    /// Get the path to the safe-coder binary
    pub fn safe_coder_binary(&self) -> PathBuf {
        // Look for the binary in target/debug or target/release
        let exe_name = if cfg!(windows) { "safe-coder.exe" } else { "safe-coder" };
        
        // Get current working directory (where cargo test is running from)
        let cwd = std::env::current_dir().expect("Failed to get current directory");
        
        // Try debug first, then release
        for target in &["debug", "release"] {
            let path = cwd.join("target").join(target).join(exe_name);
            if path.exists() {
                return path;
            }
        }
        
        // Fallback to just the name (assuming it's in PATH)
        PathBuf::from(exe_name)
    }

    /// Run safe-coder with the given arguments
    pub async fn run_safe_coder(&self, args: &[&str]) -> Result<std::process::Output> {
        let binary = self.safe_coder_binary();
        
        Command::new(binary)
            .args(args)
            .current_dir(&self.project_path)
            .env("XDG_CONFIG_HOME", self.config_dir.path())
            .env("NO_COLOR", "1") // Disable color output for easier testing
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to run safe-coder: {}", e))
    }

    /// Create a test project with some basic files
    pub fn setup_test_project(&self) -> Result<()> {
        // Create a basic Rust project structure
        self.temp_dir.child("Cargo.toml").write_str(
            r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
        )?;

        self.temp_dir.child("src").create_dir_all()?;
        self.temp_dir.child("src/main.rs").write_str(
            r#"fn main() {
    println!("Hello, world!");
}"#,
        )?;

        self.temp_dir.child("README.md").write_str(
            "# Test Project\n\nA simple test project for safe-coder integration tests.\n",
        )?;

        Ok(())
    }

    /// Initialize git repository
    pub async fn init_git(&self) -> Result<()> {
        Command::new("git")
            .args(&["init"])
            .current_dir(&self.project_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;

        Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(&self.project_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;

        Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(&self.project_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await?;

        Ok(())
    }

    /// Create a test configuration file
    pub fn create_test_config(&self) -> Result<()> {
        let config = Config {
            llm: LlmConfig {
                provider: LlmProvider::Anthropic,
                model: "test-model".to_string(),
                api_key: Some("test-key".to_string()),
                max_tokens: 1000,
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
        };

        let config_path = self.config_dir.path().join("safe-coder").join("config.toml");
        std::fs::create_dir_all(config_path.parent().unwrap())?;
        std::fs::write(&config_path, toml::to_string_pretty(&config)?)?;

        Ok(())
    }
}

/// Mock LLM response for testing
pub struct MockLlmResponse {
    pub content: String,
    pub tool_calls: Vec<String>,
}

impl Default for MockLlmResponse {
    fn default() -> Self {
        Self {
            content: "I'll help you with that task.".to_string(),
            tool_calls: vec![],
        }
    }
}

/// Assert that a string contains the given text (case-insensitive)
pub fn assert_contains(text: &str, needle: &str) {
    assert!(
        text.to_lowercase().contains(&needle.to_lowercase()),
        "Expected '{}' to contain '{}'",
        text,
        needle
    );
}

/// Assert that a command output is successful
pub fn assert_success(output: &std::process::Output) {
    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "Command failed with exit code {}\nStdout: {}\nStderr: {}",
            output.status.code().unwrap_or(-1),
            stdout,
            stderr
        );
    }
}

/// Convert process output to a string
pub fn output_to_string(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Convert process stderr to a string
pub fn stderr_to_string(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}