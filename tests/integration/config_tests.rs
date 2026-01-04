use super::common::*;
use anyhow::Result;
use safe_coder::config::{
    Config, LlmConfig, LlmProvider, OrchestratorConfig, ToolConfig, 
    GitConfig, CacheConfig, CheckpointConfig
};
use serial_test::serial;
use std::fs;
use std::env;

#[tokio::test]
#[serial]
async fn test_config_load_default() -> Result<()> {
    let _env = TestEnvironment::new()?;
    
    // Clear environment variables
    env::remove_var("ANTHROPIC_API_KEY");
    env::remove_var("OPENAI_API_KEY");
    env::remove_var("GITHUB_COPILOT_TOKEN");
    
    let config = Config::load()?;
    
    // Should load default configuration
    assert_eq!(config.llm.provider, LlmProvider::Anthropic);
    assert!(config.git.auto_commit);
    assert_eq!(config.orchestrator.max_workers, 3);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_config_load_from_file() -> Result<()> {
    let env = TestEnvironment::new()?;
    env.create_test_config()?;
    
    let config = Config::load()?;
    
    // Should load from file
    assert_eq!(config.llm.model, "test-model");
    assert_eq!(config.llm.api_key.unwrap(), "test-key");
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_config_save() -> Result<()> {
    let env = TestEnvironment::new()?;
    
    let config = Config {
        llm: LlmConfig {
            provider: LlmProvider::OpenAI,
            model: "gpt-4-turbo".to_string(),
            api_key: Some("new-api-key".to_string()),
            max_tokens: 2000,
            claude_code_oauth_compat: false,
        },
        git: GitConfig {
            auto_commit: false,
        },
        orchestrator: OrchestratorConfig::default(),
        tools: ToolConfig::default(),
        lsp: safe_coder::config::LspConfigWrapper::default(),
        cache: CacheConfig::default(),
        mcp: safe_coder::mcp::McpConfig::default(),
        checkpoint: CheckpointConfig::default(),
    };
    
    // Save and reload
    config.save()?;
    let loaded_config = Config::load()?;
    
    assert_eq!(loaded_config.llm.provider, LlmProvider::OpenAI);
    assert_eq!(loaded_config.llm.model, "gpt-4-turbo");
    assert_eq!(loaded_config.llm.api_key.unwrap(), "new-api-key");
    assert!(!loaded_config.git.auto_commit);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_config_environment_variables() -> Result<()> {
    let _env = TestEnvironment::new()?;
    
    // Set environment variables
    env::set_var("ANTHROPIC_API_KEY", "env-anthropic-key");
    env::set_var("OPENAI_API_KEY", "env-openai-key");
    
    let config = Config::default();
    
    // Should detect provider from environment
    // The exact behavior depends on the implementation
    match config.llm.provider {
        LlmProvider::Anthropic => {
            assert_eq!(config.llm.api_key.unwrap(), "env-anthropic-key");
        }
        LlmProvider::OpenAI => {
            assert_eq!(config.llm.api_key.unwrap(), "env-openai-key");
        }
        _ => {
            // Other providers might be selected based on environment
        }
    }
    
    // Clean up
    env::remove_var("ANTHROPIC_API_KEY");
    env::remove_var("OPENAI_API_KEY");
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_config_validation() -> Result<()> {
    let _env = TestEnvironment::new()?;
    
    // Test invalid configuration values
    let invalid_config = Config {
        llm: LlmConfig {
            provider: LlmProvider::Anthropic,
            model: "".to_string(), // Empty model
            api_key: None,
            max_tokens: 0, // Invalid token count
            claude_code_oauth_compat: false,
        },
        git: GitConfig {
            auto_commit: true,
        },
        orchestrator: OrchestratorConfig::default(),
        tools: ToolConfig::default(),
        lsp: safe_coder::config::LspConfigWrapper::default(),
        cache: CacheConfig::default(),
        mcp: safe_coder::mcp::McpConfig::default(),
        checkpoint: CheckpointConfig::default(),
    };
    
    // Configuration should handle invalid values gracefully
    // This depends on whether validation is implemented
    let toml_result = toml::to_string_pretty(&invalid_config);
    assert!(toml_result.is_ok());
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_config_provider_switching() -> Result<()> {
    let _env = TestEnvironment::new()?;
    
    let providers = vec![
        LlmProvider::Anthropic,
        LlmProvider::OpenAI,
        LlmProvider::Ollama,
        LlmProvider::GitHubCopilot,
    ];
    
    for provider in providers {
        let config = Config {
            llm: LlmConfig {
                provider: provider.clone(),
                model: "test-model".to_string(),
                api_key: Some("test-key".to_string()),
                max_tokens: 1000,
                claude_code_oauth_compat: false,
            },
            git: GitConfig::default(),
            orchestrator: OrchestratorConfig::default(),
            tools: ToolConfig::default(),
            lsp: safe_coder::config::LspConfigWrapper::default(),
            cache: CacheConfig::default(),
            mcp: safe_coder::mcp::McpConfig::default(),
            checkpoint: CheckpointConfig::default(),
        };
        
        // Should serialize/deserialize correctly
        let toml_str = toml::to_string_pretty(&config)?;
        let deserialized: Config = toml::from_str(&toml_str)?;
        
        assert_eq!(deserialized.llm.provider, provider);
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_orchestrator_config() -> Result<()> {
    let _env = TestEnvironment::new()?;
    
    let config = Config {
        llm: LlmConfig::default(),
        git: GitConfig::default(),
        orchestrator: OrchestratorConfig {
            claude_cli_path: "custom-claude-path".to_string(),
            gemini_cli_path: "custom-gemini-path".to_string(),
            safe_coder_cli_path: "custom-safe-coder-path".to_string(),
            gh_cli_path: "custom-gh-path".to_string(),
            max_workers: 5,
            default_worker: "gemini".to_string(),
            worker_strategy: "round-robin".to_string(),
            enabled_workers: vec!["claude".to_string(), "gemini".to_string()],
            use_worktrees: false,
            throttle_limits: safe_coder::config::ThrottleLimitsConfig {
                claude_max_concurrent: 3,
                gemini_max_concurrent: 4,
                safe_coder_max_concurrent: 2,
                copilot_max_concurrent: 2,
                start_delay_ms: 200,
            },
        },
        tools: ToolConfig::default(),
        lsp: safe_coder::config::LspConfigWrapper::default(),
        cache: CacheConfig::default(),
        mcp: safe_coder::mcp::McpConfig::default(),
        checkpoint: CheckpointConfig::default(),
    };
    
    // Test serialization/deserialization
    let toml_str = toml::to_string_pretty(&config)?;
    let deserialized: Config = toml::from_str(&toml_str)?;
    
    assert_eq!(deserialized.orchestrator.max_workers, 5);
    assert_eq!(deserialized.orchestrator.default_worker, "gemini");
    assert_eq!(deserialized.orchestrator.worker_strategy, "round-robin");
    assert!(!deserialized.orchestrator.use_worktrees);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_tool_config() -> Result<()> {
    let _env = TestEnvironment::new()?;
    
    let config = Config {
        llm: LlmConfig::default(),
        git: GitConfig::default(),
        orchestrator: OrchestratorConfig::default(),
        tools: ToolConfig {
            bash_timeout_secs: 300,
            max_output_bytes: 2_097_152,
            warn_dangerous_commands: false,
            dangerous_patterns: vec![
                "rm -rf *".to_string(),
                "format".to_string(),
                "mkfs.*".to_string(),
            ],
        },
        lsp: safe_coder::config::LspConfigWrapper::default(),
        cache: CacheConfig::default(),
        mcp: safe_coder::mcp::McpConfig::default(),
        checkpoint: CheckpointConfig::default(),
    };
    
    // Test serialization/deserialization
    let toml_str = toml::to_string_pretty(&config)?;
    let deserialized: Config = toml::from_str(&toml_str)?;
    
    assert_eq!(deserialized.tools.bash_timeout_secs, 300);
    assert_eq!(deserialized.tools.max_output_bytes, 2_097_152);
    assert!(!deserialized.tools.warn_dangerous_commands);
    assert_eq!(deserialized.tools.dangerous_patterns.len(), 3);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_cache_config() -> Result<()> {
    let _env = TestEnvironment::new()?;
    
    let config = Config {
        llm: LlmConfig::default(),
        git: GitConfig::default(),
        orchestrator: OrchestratorConfig::default(),
        tools: ToolConfig::default(),
        lsp: safe_coder::config::LspConfigWrapper::default(),
        cache: CacheConfig {
            enabled: true,
            max_size_mb: 1024,
            ttl_hours: 48,
            compression: true,
        },
        mcp: safe_coder::mcp::McpConfig::default(),
        checkpoint: CheckpointConfig::default(),
    };
    
    // Test serialization/deserialization
    let toml_str = toml::to_string_pretty(&config)?;
    let deserialized: Config = toml::from_str(&toml_str)?;
    
    assert!(deserialized.cache.enabled);
    assert_eq!(deserialized.cache.max_size_mb, 1024);
    assert_eq!(deserialized.cache.ttl_hours, 48);
    assert!(deserialized.cache.compression);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_checkpoint_config() -> Result<()> {
    let _env = TestEnvironment::new()?;
    
    let config = Config {
        llm: LlmConfig::default(),
        git: GitConfig::default(),
        orchestrator: OrchestratorConfig::default(),
        tools: ToolConfig::default(),
        lsp: safe_coder::config::LspConfigWrapper::default(),
        cache: CacheConfig::default(),
        mcp: safe_coder::mcp::McpConfig::default(),
        checkpoint: CheckpointConfig {
            enabled: false,
            max_checkpoints: 20,
            storage_path: Some("/custom/checkpoint/path".to_string()),
            ignore_patterns: vec![
                "target/".to_string(),
                "node_modules/".to_string(),
                "*.log".to_string(),
            ],
        },
    };
    
    // Test serialization/deserialization
    let toml_str = toml::to_string_pretty(&config)?;
    let deserialized: Config = toml::from_str(&toml_str)?;
    
    assert!(!deserialized.checkpoint.enabled);
    assert_eq!(deserialized.checkpoint.max_checkpoints, 20);
    assert_eq!(
        deserialized.checkpoint.storage_path.unwrap(),
        "/custom/checkpoint/path"
    );
    assert_eq!(deserialized.checkpoint.ignore_patterns.len(), 3);
    
    Ok(())
}

#[cfg(test)]
mod config_file_handling_tests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_config_file_creation() -> Result<()> {
        let env = TestEnvironment::new()?;
        
        let config = Config::default();
        config.save()?;
        
        // Verify config file was created
        let config_path = Config::config_path()?;
        assert!(config_path.exists());
        
        // Verify file contains expected content
        let content = fs::read_to_string(config_path)?;
        assert_contains(&content, "[llm]");
        assert_contains(&content, "[git]");
        assert_contains(&content, "[orchestrator]");
        
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_config_file_malformed() -> Result<()> {
        let env = TestEnvironment::new()?;
        
        // Create a malformed config file
        let config_path = Config::config_path()?;
        fs::create_dir_all(config_path.parent().unwrap())?;
        fs::write(&config_path, "invalid toml content [")?;
        
        // Should handle malformed config gracefully
        let result = Config::load();
        
        match result {
            Ok(_) => {
                // Might fall back to default config
            }
            Err(e) => {
                // Should provide helpful error message
                assert_contains(&e.to_string(), "config");
            }
        }
        
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_config_permission_errors() -> Result<()> {
        let env = TestEnvironment::new()?;
        
        // This test would require setting up permission-denied scenarios
        // which is complex to do in a portable way
        
        // For now, just test that config operations don't panic
        let config = Config::default();
        let save_result = config.save();
        
        match save_result {
            Ok(_) => {
                // Success case
            }
            Err(e) => {
                // Should handle errors gracefully
                assert!(!e.to_string().contains("panic"));
            }
        }
        
        Ok(())
    }
}

#[cfg(test)]
mod config_migration_tests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_config_backwards_compatibility() -> Result<()> {
        let env = TestEnvironment::new()?;
        
        // Create an old-style config (minimal)
        let old_config = r#"
[llm]
provider = "Anthropic"
model = "claude-3-haiku"
api_key = "old-api-key"
max_tokens = 4096
"#;
        
        let config_path = Config::config_path()?;
        fs::create_dir_all(config_path.parent().unwrap())?;
        fs::write(&config_path, old_config)?;
        
        // Should load with defaults for missing sections
        let loaded_config = Config::load()?;
        
        assert_eq!(loaded_config.llm.model, "claude-3-haiku");
        assert_eq!(loaded_config.llm.api_key.unwrap(), "old-api-key");
        // Missing sections should use defaults
        assert!(loaded_config.git.auto_commit); // Default value
        
        Ok(())
    }
}