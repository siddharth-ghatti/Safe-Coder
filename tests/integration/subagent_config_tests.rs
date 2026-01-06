//! Integration tests for Subagent Configuration
//!
//! Tests the per-subagent LLM provider/model configuration functionality.

use anyhow::Result;
use serial_test::serial;
use safe_coder::config::{Config, SubagentConfig, SubagentModelConfig, LlmProvider};

#[test]
fn test_subagent_config_default() {
    let config = SubagentConfig::default();

    assert!(config.analyzer.is_none());
    assert!(config.tester.is_none());
    assert!(config.refactorer.is_none());
    assert!(config.documenter.is_none());
    assert!(config.custom.is_none());
}

#[test]
fn test_subagent_config_with_analyzer() {
    let config = SubagentConfig {
        analyzer: Some(SubagentModelConfig {
            provider: LlmProvider::Anthropic,
            model: "claude-3.5-sonnet".to_string(),
            api_key: None,
            max_tokens: 4096,
        }),
        tester: None,
        refactorer: None,
        documenter: None,
        custom: None,
    };

    assert!(config.analyzer.is_some());
    let analyzer = config.analyzer.as_ref().unwrap();
    assert_eq!(analyzer.provider, LlmProvider::Anthropic);
    assert_eq!(analyzer.model, "claude-3.5-sonnet");
}

#[test]
fn test_subagent_model_config_with_explicit_api_key() {
    let config = SubagentModelConfig {
        provider: LlmProvider::OpenAI,
        model: "gpt-4o".to_string(),
        api_key: Some("sk-test-key".to_string()),
        max_tokens: 8192,
    };

    let key = config.get_api_key();
    assert_eq!(key, Some("sk-test-key".to_string()));
}

#[test]
fn test_subagent_model_config_ollama_no_key() {
    let config = SubagentModelConfig {
        provider: LlmProvider::Ollama,
        model: "llama2".to_string(),
        api_key: None,
        max_tokens: 4096,
    };

    // Ollama doesn't need an API key
    let key = config.get_api_key();
    assert!(key.is_none());
}

#[test]
fn test_config_get_subagent_model_analyzer() -> Result<()> {
    let mut config = Config::default();
    config.subagents.analyzer = Some(SubagentModelConfig {
        provider: LlmProvider::OpenAI,
        model: "gpt-4o".to_string(),
        api_key: Some("test-key".to_string()),
        max_tokens: 8192,
    });

    // Should return the analyzer config
    let model = config.get_subagent_model("analyzer");
    assert!(model.is_some());
    assert_eq!(model.unwrap().model, "gpt-4o");

    // "code_analyzer" should also work
    let model = config.get_subagent_model("code_analyzer");
    assert!(model.is_some());

    Ok(())
}

#[test]
fn test_config_get_subagent_model_tester() -> Result<()> {
    let mut config = Config::default();
    config.subagents.tester = Some(SubagentModelConfig {
        provider: LlmProvider::Anthropic,
        model: "claude-3-haiku".to_string(),
        api_key: None,
        max_tokens: 2048,
    });

    let model = config.get_subagent_model("tester");
    assert!(model.is_some());
    assert_eq!(model.unwrap().model, "claude-3-haiku");

    Ok(())
}

#[test]
fn test_config_get_subagent_model_refactorer() -> Result<()> {
    let mut config = Config::default();
    config.subagents.refactorer = Some(SubagentModelConfig {
        provider: LlmProvider::OpenRouter,
        model: "anthropic/claude-3.5-sonnet".to_string(),
        api_key: Some("openrouter-key".to_string()),
        max_tokens: 4096,
    });

    let model = config.get_subagent_model("refactorer");
    assert!(model.is_some());
    let model = model.unwrap();
    assert_eq!(model.provider, LlmProvider::OpenRouter);
    assert_eq!(model.model, "anthropic/claude-3.5-sonnet");

    Ok(())
}

#[test]
fn test_config_get_subagent_model_documenter() -> Result<()> {
    let mut config = Config::default();
    config.subagents.documenter = Some(SubagentModelConfig {
        provider: LlmProvider::OpenAI,
        model: "gpt-4o-mini".to_string(),
        api_key: None,
        max_tokens: 2048,
    });

    let model = config.get_subagent_model("documenter");
    assert!(model.is_some());
    assert_eq!(model.unwrap().model, "gpt-4o-mini");

    Ok(())
}

#[test]
fn test_config_get_subagent_model_custom() -> Result<()> {
    let mut config = Config::default();
    config.subagents.custom = Some(SubagentModelConfig {
        provider: LlmProvider::Ollama,
        model: "codellama".to_string(),
        api_key: None,
        max_tokens: 4096,
    });

    let model = config.get_subagent_model("custom");
    assert!(model.is_some());
    let model = model.unwrap();
    assert_eq!(model.provider, LlmProvider::Ollama);
    assert_eq!(model.model, "codellama");

    Ok(())
}

#[test]
fn test_config_get_subagent_model_fallback() -> Result<()> {
    let config = Config::default();

    // When no subagent config is set, should return None
    let model = config.get_subagent_model("analyzer");
    assert!(model.is_none());

    let model = config.get_subagent_model("tester");
    assert!(model.is_none());

    Ok(())
}

#[test]
fn test_config_get_subagent_model_unknown() -> Result<()> {
    let config = Config::default();

    // Unknown subagent type should return None
    let model = config.get_subagent_model("unknown");
    assert!(model.is_none());

    let model = config.get_subagent_model("invalid");
    assert!(model.is_none());

    Ok(())
}

#[test]
fn test_subagent_config_serialization() -> Result<()> {
    let config = SubagentConfig {
        analyzer: Some(SubagentModelConfig {
            provider: LlmProvider::Anthropic,
            model: "claude-3.5-sonnet".to_string(),
            api_key: Some("test-key".to_string()),
            max_tokens: 4096,
        }),
        tester: Some(SubagentModelConfig {
            provider: LlmProvider::OpenAI,
            model: "gpt-4o".to_string(),
            api_key: None,
            max_tokens: 8192,
        }),
        refactorer: None,
        documenter: None,
        custom: None,
    };

    let serialized = toml::to_string_pretty(&config)?;

    // Verify serialization contains expected fields
    assert!(serialized.contains("[analyzer]"));
    assert!(serialized.contains("claude-3.5-sonnet"));
    assert!(serialized.contains("[tester]"));
    assert!(serialized.contains("gpt-4o"));

    Ok(())
}

#[test]
fn test_subagent_config_deserialization() -> Result<()> {
    let toml_str = r#"
[analyzer]
provider = "anthropic"
model = "claude-3.5-sonnet"
max_tokens = 4096

[tester]
provider = "openai"
model = "gpt-4o"
api_key = "test-key"
max_tokens = 8192
"#;

    let config: SubagentConfig = toml::from_str(toml_str)?;

    assert!(config.analyzer.is_some());
    let analyzer = config.analyzer.unwrap();
    assert_eq!(analyzer.provider, LlmProvider::Anthropic);
    assert_eq!(analyzer.model, "claude-3.5-sonnet");

    assert!(config.tester.is_some());
    let tester = config.tester.unwrap();
    assert_eq!(tester.provider, LlmProvider::OpenAI);
    assert_eq!(tester.model, "gpt-4o");
    assert_eq!(tester.api_key, Some("test-key".to_string()));

    Ok(())
}

#[test]
fn test_llm_provider_openrouter() {
    let provider = LlmProvider::OpenRouter;

    // Test serialization
    let serialized = serde_json::to_string(&provider).unwrap();
    assert_eq!(serialized, "\"openrouter\"");

    // Test deserialization
    let deserialized: LlmProvider = serde_json::from_str("\"openrouter\"").unwrap();
    assert_eq!(deserialized, LlmProvider::OpenRouter);
}

#[test]
fn test_llm_provider_all_variants() -> Result<()> {
    // Test all provider variants can be serialized/deserialized
    let providers = vec![
        LlmProvider::Anthropic,
        LlmProvider::OpenAI,
        LlmProvider::Ollama,
        LlmProvider::GitHubCopilot,
        LlmProvider::OpenRouter,
    ];

    for provider in providers {
        let serialized = serde_json::to_string(&provider)?;
        let deserialized: LlmProvider = serde_json::from_str(&serialized)?;
        assert_eq!(deserialized, provider);
    }

    Ok(())
}

#[test]
fn test_subagent_model_config_default_max_tokens() -> Result<()> {
    let toml_str = r#"
provider = "anthropic"
model = "claude-3.5-sonnet"
"#;

    let config: SubagentModelConfig = toml::from_str(toml_str)?;

    // Should use default max_tokens of 4096
    assert_eq!(config.max_tokens, 4096);

    Ok(())
}

#[test]
fn test_full_config_with_subagents() -> Result<()> {
    let toml_str = r#"
[llm]
provider = "anthropic"
model = "claude-3.5-sonnet"
max_tokens = 8192

[subagents.analyzer]
provider = "openai"
model = "gpt-4o-mini"
max_tokens = 2048

[subagents.tester]
provider = "anthropic"
model = "claude-3-haiku"
max_tokens = 4096
"#;

    let config: Config = toml::from_str(toml_str)?;

    // Check main LLM config
    assert_eq!(config.llm.provider, LlmProvider::Anthropic);
    assert_eq!(config.llm.model, "claude-3.5-sonnet");

    // Check subagent configs
    assert!(config.subagents.analyzer.is_some());
    assert!(config.subagents.tester.is_some());
    assert!(config.subagents.refactorer.is_none());

    let analyzer = config.subagents.analyzer.unwrap();
    assert_eq!(analyzer.provider, LlmProvider::OpenAI);
    assert_eq!(analyzer.model, "gpt-4o-mini");

    Ok(())
}

#[test]
fn test_subagent_config_empty_is_valid() -> Result<()> {
    let toml_str = r#"
[llm]
provider = "anthropic"
model = "claude-3.5-sonnet"
max_tokens = 8192
"#;

    let config: Config = toml::from_str(toml_str)?;

    // Subagents should all be None (using default)
    assert!(config.subagents.analyzer.is_none());
    assert!(config.subagents.tester.is_none());
    assert!(config.subagents.refactorer.is_none());
    assert!(config.subagents.documenter.is_none());
    assert!(config.subagents.custom.is_none());

    Ok(())
}
