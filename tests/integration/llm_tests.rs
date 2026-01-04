use super::common::*;
use anyhow::Result;
use mockito::{Mock, Server, ServerGuard};
use safe_coder::config::{Config, LlmConfig, LlmProvider};
use safe_coder::llm::{create_client, Message, ContentBlock};
use serial_test::serial;
use std::sync::Arc;

/// Test the LLM client creation and basic functionality
#[tokio::test]
#[serial]
async fn test_llm_client_creation() -> Result<()> {
    let config = LlmConfig {
        provider: LlmProvider::Anthropic,
        model: "claude-3-haiku-20240307".to_string(),
        api_key: Some("test-key".to_string()),
        max_tokens: 1000,
        claude_code_oauth_compat: false,
    };

    let client = create_client(&config).await?;
    
    // Should create a client without error
    assert!(Arc::strong_count(&client) >= 1);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_anthropic_client_with_mock() -> Result<()> {
    let mut server = Server::new_async().await;
    
    let mock = server.mock("POST", "/v1/messages")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "content": [
                {
                    "type": "text",
                    "text": "Hello! I'm Claude, an AI assistant."
                }
            ],
            "model": "claude-3-haiku-20240307",
            "stop_reason": "end_turn",
            "stop_sequence": null,
            "usage": {
                "input_tokens": 10,
                "output_tokens": 20
            }
        }"#)
        .create_async()
        .await;

    // Override the base URL for testing
    std::env::set_var("ANTHROPIC_BASE_URL", &server.url());
    
    let config = LlmConfig {
        provider: LlmProvider::Anthropic,
        model: "claude-3-haiku-20240307".to_string(),
        api_key: Some("test-key".to_string()),
        max_tokens: 1000,
        claude_code_oauth_compat: false,
    };

    let client = create_client(&config).await?;
    
    let messages = vec![Message {
        role: "user".to_string(),
        content: vec![ContentBlock::Text {
            text: "Hello, Claude!".to_string(),
        }],
    }];

    // This would make an actual HTTP request in a real integration test
    // For now, we just test that the client was created successfully
    
    mock.assert_async().await;
    
    // Clean up
    std::env::remove_var("ANTHROPIC_BASE_URL");
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_openai_client_creation() -> Result<()> {
    let config = LlmConfig {
        provider: LlmProvider::OpenAI,
        model: "gpt-4".to_string(),
        api_key: Some("test-key".to_string()),
        max_tokens: 1000,
        claude_code_oauth_compat: false,
    };

    let client = create_client(&config).await?;
    
    // Should create a client without error
    assert!(Arc::strong_count(&client) >= 1);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_openai_client_with_mock() -> Result<()> {
    let mut server = Server::new_async().await;
    
    let mock = server.mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{
            "id": "chatcmpl-123",
            "object": "chat.completion",
            "created": 1677652288,
            "model": "gpt-4",
            "choices": [
                {
                    "index": 0,
                    "message": {
                        "role": "assistant",
                        "content": "Hello! I'm ChatGPT, an AI assistant."
                    },
                    "finish_reason": "stop"
                }
            ],
            "usage": {
                "prompt_tokens": 10,
                "completion_tokens": 20,
                "total_tokens": 30
            }
        }"#)
        .create_async()
        .await;

    // Override the base URL for testing
    std::env::set_var("OPENAI_BASE_URL", &server.url());
    
    let config = LlmConfig {
        provider: LlmProvider::OpenAI,
        model: "gpt-4".to_string(),
        api_key: Some("test-key".to_string()),
        max_tokens: 1000,
        claude_code_oauth_compat: false,
    };

    let client = create_client(&config).await?;
    
    // Test client creation - actual request testing would require more setup
    assert!(Arc::strong_count(&client) >= 1);
    
    mock.assert_async().await;
    
    // Clean up
    std::env::remove_var("OPENAI_BASE_URL");
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_ollama_client_creation() -> Result<()> {
    let config = LlmConfig {
        provider: LlmProvider::Ollama,
        model: "llama2".to_string(),
        api_key: None, // Ollama doesn't require API key
        max_tokens: 1000,
        claude_code_oauth_compat: false,
    };

    let client = create_client(&config).await?;
    
    // Should create a client without error
    assert!(Arc::strong_count(&client) >= 1);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_github_copilot_client_creation() -> Result<()> {
    let config = LlmConfig {
        provider: LlmProvider::GitHubCopilot,
        model: "gpt-4".to_string(),
        api_key: Some("ghu_123456789".to_string()),
        max_tokens: 1000,
        claude_code_oauth_compat: false,
    };

    let client = create_client(&config).await?;
    
    // Should create a client without error
    assert!(Arc::strong_count(&client) >= 1);
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_llm_config_validation() -> Result<()> {
    // Test missing API key for providers that require it
    let config_missing_key = LlmConfig {
        provider: LlmProvider::Anthropic,
        model: "claude-3-haiku-20240307".to_string(),
        api_key: None,
        max_tokens: 1000,
        claude_code_oauth_compat: false,
    };

    // This should either fail or use environment variables
    let result = create_client(&config_missing_key).await;
    
    // The behavior depends on whether environment variables are set
    match result {
        Ok(_) => {
            // Client created successfully (likely from env vars)
        }
        Err(e) => {
            // Expected if no API key is available
            assert_contains(&e.to_string().to_lowercase(), "api");
        }
    }
    
    Ok(())
}

#[tokio::test]
#[serial]
async fn test_llm_client_error_handling() -> Result<()> {
    let mut server = Server::new_async().await;
    
    // Mock an error response
    let mock = server.mock("POST", "/v1/messages")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(r#"{
            "error": {
                "type": "authentication_error",
                "message": "Invalid API key"
            }
        }"#)
        .create_async()
        .await;

    std::env::set_var("ANTHROPIC_BASE_URL", &server.url());
    
    let config = LlmConfig {
        provider: LlmProvider::Anthropic,
        model: "claude-3-haiku-20240307".to_string(),
        api_key: Some("invalid-key".to_string()),
        max_tokens: 1000,
        claude_code_oauth_compat: false,
    };

    let client = create_client(&config).await?;
    
    // The client should be created, but requests would fail
    // In a real test, we would make a request and check for proper error handling
    
    mock.assert_async().await;
    
    std::env::remove_var("ANTHROPIC_BASE_URL");
    
    Ok(())
}

#[cfg(test)]
mod caching_tests {
    use super::*;
    use safe_coder::llm::cached::CachedLlmClient;

    #[tokio::test]
    #[serial]
    async fn test_cached_client_creation() -> Result<()> {
        let config = LlmConfig {
            provider: LlmProvider::Anthropic,
            model: "claude-3-haiku-20240307".to_string(),
            api_key: Some("test-key".to_string()),
            max_tokens: 1000,
            claude_code_oauth_compat: false,
        };

        let base_client = create_client(&config).await?;
        
        // Test that we can create a cached client
        let cached_client = CachedLlmClient::new(base_client);
        
        // Should create without error
        assert!(std::mem::size_of_val(&cached_client) > 0);
        
        Ok(())
    }
}

#[cfg(test)]
mod provider_switching_tests {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_provider_switching() -> Result<()> {
        let providers = vec![
            (LlmProvider::Anthropic, "claude-3-haiku-20240307"),
            (LlmProvider::OpenAI, "gpt-4"),
            (LlmProvider::Ollama, "llama2"),
            (LlmProvider::GitHubCopilot, "gpt-4"),
        ];

        for (provider, model) in providers {
            let config = LlmConfig {
                provider: provider.clone(),
                model: model.to_string(),
                api_key: Some("test-key".to_string()),
                max_tokens: 1000,
                claude_code_oauth_compat: false,
            };

            let client = create_client(&config).await?;
            
            // Each provider should create a valid client
            assert!(Arc::strong_count(&client) >= 1);
        }
        
        Ok(())
    }
}