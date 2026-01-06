use anyhow::Result;
use safe_coder::config::{
    Config, LlmProvider, OrchestratorConfig, ThrottleLimitsConfig, ToolConfig,
};
use std::fs;
use std::sync::Mutex;
use tempfile::TempDir;

// Mutex to serialize tests that modify environment variables
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// Helper to clear all API key environment variables
fn clear_api_env_vars() {
    std::env::remove_var("ANTHROPIC_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    std::env::remove_var("GITHUB_COPILOT_TOKEN");
}

#[test]
fn test_config_default() {
    let _guard = ENV_MUTEX.lock().unwrap();
    clear_api_env_vars();

    let config = Config::default();

    // Test LLM config defaults (without environment variables, defaults to Anthropic)
    assert_eq!(config.llm.provider, LlmProvider::Anthropic);
    assert_eq!(config.llm.model, "claude-sonnet-4-20250514");
    assert_eq!(config.llm.max_tokens, 8192);
    assert!(config.llm.api_key.is_none());
    assert!(!config.llm.claude_code_oauth_compat);

    // Test Git config defaults
    assert!(config.git.auto_commit);

    // Test Orchestrator config defaults
    assert_eq!(config.orchestrator.max_workers, 3);
    assert_eq!(config.orchestrator.default_worker, "claude");
    assert_eq!(config.orchestrator.worker_strategy, "single");
    assert!(config.orchestrator.use_worktrees);

    // Test Tool config defaults
    assert_eq!(config.tools.bash_timeout_secs, 120);
    assert_eq!(config.tools.max_output_bytes, 1_048_576);
    assert!(config.tools.warn_dangerous_commands);
    assert!(!config.tools.dangerous_patterns.is_empty());

    // Test Checkpoint config defaults
    assert!(config.checkpoint.enabled);
    assert_eq!(config.checkpoint.max_checkpoints, 10);
    assert!(config.checkpoint.storage_path.is_none());
    assert!(!config.checkpoint.ignore_patterns.is_empty());
}

#[test]
fn test_config_serialization() -> Result<()> {
    let config = Config::default();

    // Test serialization to TOML
    let toml_str = toml::to_string_pretty(&config)?;
    assert!(toml_str.contains("[llm]"));
    assert!(toml_str.contains("[git]"));
    assert!(toml_str.contains("[orchestrator]"));
    assert!(toml_str.contains("[tools]"));

    // Test deserialization from TOML
    let deserialized: Config = toml::from_str(&toml_str)?;
    assert_eq!(config.llm.provider, deserialized.llm.provider);
    assert_eq!(config.llm.model, deserialized.llm.model);
    assert_eq!(config.git.auto_commit, deserialized.git.auto_commit);

    Ok(())
}

#[test]
fn test_config_load_nonexistent() -> Result<()> {
    // Test loading config when file doesn't exist - should return default
    let config = Config::load()?;
    let default_config = Config::default();

    assert_eq!(config.llm.provider, default_config.llm.provider);
    assert_eq!(config.llm.model, default_config.llm.model);

    Ok(())
}

#[test]
fn test_config_save_load_roundtrip() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let config_path = temp_dir.path().join("config.toml");

    // Create a custom config
    let mut config = Config::default();
    config.llm.model = "test-model".to_string();
    config.llm.max_tokens = 4096;
    config.git.auto_commit = false;
    config.orchestrator.max_workers = 5;

    // Serialize to TOML and write to file
    let toml_str = toml::to_string_pretty(&config)?;
    fs::write(&config_path, toml_str)?;

    // Load from file and verify
    let content = fs::read_to_string(&config_path)?;
    let loaded_config: Config = toml::from_str(&content)?;

    assert_eq!(config.llm.model, loaded_config.llm.model);
    assert_eq!(config.llm.max_tokens, loaded_config.llm.max_tokens);
    assert_eq!(config.git.auto_commit, loaded_config.git.auto_commit);
    assert_eq!(
        config.orchestrator.max_workers,
        loaded_config.orchestrator.max_workers
    );

    Ok(())
}

#[test]
fn test_llm_provider_serialization() -> Result<()> {
    let _guard = ENV_MUTEX.lock().unwrap();
    clear_api_env_vars();

    // Test provider serialization through a full config roundtrip
    // (TOML can't serialize bare enums, but works when embedded in a struct)

    let providers_and_models = vec![
        (LlmProvider::Anthropic, "claude-sonnet-4-20250514"),
        (LlmProvider::OpenAI, "gpt-4o"),
        (LlmProvider::Ollama, "llama2"),
        (LlmProvider::GitHubCopilot, "gpt-4"),
    ];

    for (provider, model) in providers_and_models {
        let mut config = Config::default();
        config.llm.provider = provider.clone();
        config.llm.model = model.to_string();

        let serialized = toml::to_string_pretty(&config)?;
        let deserialized: Config = toml::from_str(&serialized)?;

        assert_eq!(deserialized.llm.provider, provider);
        assert_eq!(deserialized.llm.model, model);
    }

    Ok(())
}

#[test]
fn test_tool_config_dangerous_patterns() {
    let config = ToolConfig::default();

    // Test that dangerous patterns are properly configured
    assert!(!config.dangerous_patterns.is_empty());

    // Test some basic dangerous patterns are included
    let patterns = config.dangerous_patterns.join(" ");
    assert!(patterns.contains("rm"));
    assert!(patterns.contains("mkfs"));
    assert!(patterns.contains("dd"));
}

#[test]
fn test_throttle_limits_defaults() {
    let limits = ThrottleLimitsConfig::default();

    assert_eq!(limits.claude_max_concurrent, 2);
    assert_eq!(limits.gemini_max_concurrent, 2);
    assert_eq!(limits.safe_coder_max_concurrent, 2);
    assert_eq!(limits.copilot_max_concurrent, 2);
    assert_eq!(limits.start_delay_ms, 100);
}

#[test]
fn test_orchestrator_config_defaults() {
    let config = OrchestratorConfig::default();

    assert_eq!(config.claude_cli_path, "claude");
    assert_eq!(config.gemini_cli_path, "gemini");
    assert_eq!(config.gh_cli_path, "gh");
    assert_eq!(config.max_workers, 3);
    assert_eq!(config.default_worker, "claude");
    assert_eq!(config.worker_strategy, "single");
    assert!(config.use_worktrees);
    assert_eq!(config.enabled_workers, vec!["claude".to_string()]);
}

#[cfg(test)]
mod environment_tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_from_environment() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_api_env_vars();

        // Set environment variables
        env::set_var("ANTHROPIC_API_KEY", "test-anthropic-key");

        let config = Config::default();

        // Should detect Anthropic from environment
        assert_eq!(config.llm.provider, LlmProvider::Anthropic);
        assert_eq!(config.llm.api_key, Some("test-anthropic-key".to_string()));
        assert_eq!(config.llm.model, "claude-sonnet-4-20250514");

        // Clean up
        env::remove_var("ANTHROPIC_API_KEY");
    }

    #[test]
    fn test_config_openai_environment() {
        let _guard = ENV_MUTEX.lock().unwrap();
        clear_api_env_vars();

        // Set environment variables
        env::set_var("OPENAI_API_KEY", "test-openai-key");

        let config = Config::default();

        // Should detect OpenAI from environment
        assert_eq!(config.llm.provider, LlmProvider::OpenAI);
        assert_eq!(config.llm.api_key, Some("test-openai-key".to_string()));
        assert_eq!(config.llm.model, "gpt-4o");

        // Clean up
        env::remove_var("OPENAI_API_KEY");
    }
}
