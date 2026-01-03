use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::auth::{StoredToken, TokenManager, TokenProvider};
use crate::config::{Config, LlmConfig, LlmProvider};

pub mod anthropic;
pub mod cached;
pub mod copilot;
pub mod ollama;
pub mod openai;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Token usage information from LLM response
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    /// Tokens written to provider cache (Anthropic cache_creation_input_tokens)
    pub cache_creation_tokens: Option<usize>,
    /// Tokens read from provider cache (Anthropic cache_read_input_tokens, OpenAI cached_tokens)
    pub cache_read_tokens: Option<usize>,
}

impl TokenUsage {
    /// Create a new TokenUsage with basic token counts
    pub fn new(input_tokens: usize, output_tokens: usize) -> Self {
        Self {
            input_tokens,
            output_tokens,
            cache_creation_tokens: None,
            cache_read_tokens: None,
        }
    }

    /// Create a new TokenUsage with cache information
    pub fn with_cache(
        input_tokens: usize,
        output_tokens: usize,
        cache_creation_tokens: Option<usize>,
        cache_read_tokens: Option<usize>,
    ) -> Self {
        Self {
            input_tokens,
            output_tokens,
            cache_creation_tokens,
            cache_read_tokens,
        }
    }

    /// Check if this response had any cache hits
    pub fn has_cache_hit(&self) -> bool {
        self.cache_read_tokens.map(|t| t > 0).unwrap_or(false)
    }

    /// Get total tokens saved by caching (cache reads are 90% cheaper)
    pub fn tokens_saved_by_cache(&self) -> usize {
        // Cache reads cost 10% of normal, so we save 90% of those tokens
        self.cache_read_tokens.unwrap_or(0)
    }
}

/// Response from LLM including message and token usage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub message: Message,
    pub usage: Option<TokenUsage>,
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    /// Send a message to the LLM with optional system prompt
    /// Returns both the message and token usage
    async fn send_message_with_system(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system_prompt: Option<&str>,
    ) -> Result<LlmResponse>;

    /// Send a message to the LLM (backward compatible, no system prompt)
    async fn send_message(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        self.send_message_with_system(messages, tools, None).await
    }
}

/// Create an LLM client with optional caching wrapper
pub async fn create_client(config: &crate::config::Config) -> Result<Box<dyn LlmClient>> {
    // Create the underlying provider client
    let inner_client = create_provider_client(config).await?;

    // Wrap with caching if enabled
    if config.cache.enabled {
        let cache_config = config.cache.to_llm_cache_config();
        tracing::info!(
            "Token caching enabled (app_cache={}, provider_native={}, ttl={}min)",
            cache_config.application_cache,
            cache_config.provider_native,
            config.cache.ttl_minutes
        );
        Ok(Box::new(cached::CachingLlmClient::new(
            inner_client,
            config.llm.model.clone(),
            cache_config,
        )))
    } else {
        Ok(inner_client)
    }
}

/// Create the underlying provider-specific LLM client (without caching)
async fn create_provider_client(config: &crate::config::Config) -> Result<Box<dyn LlmClient>> {
    match config.llm.provider {
        LlmProvider::Anthropic => {
            // Check if we have a stored token (could be OAuth or API key)
            if let Some(stored_token) = config.get_stored_token() {
                // For OAuth tokens, use TokenManager for automatic refresh
                if stored_token.is_oauth() {
                    let token_path = Config::token_path(&LlmProvider::Anthropic)?;
                    let token_manager = Arc::new(TokenManager::new(
                        stored_token.clone(),
                        token_path,
                        TokenProvider::Anthropic,
                    ));

                    // Check if token needs immediate refresh
                    if stored_token.needs_refresh() {
                        tracing::info!("OAuth token expiring soon, refreshing...");
                        if let Err(e) = token_manager.refresh().await {
                            tracing::warn!(
                                "Failed to refresh token: {}. Will try with current token.",
                                e
                            );
                        }
                    }

                    if let Some(secs) = token_manager.seconds_until_expiry().await {
                        if secs > 0 {
                            tracing::info!(
                                "Using OAuth authentication for Anthropic (expires in {}m)",
                                secs / 60
                            );
                        }
                    }

                    // OAuth REQUIRES Claude Code compatibility mode - always enable it
                    // The Claude Code system prompt is required for OAuth tokens to work
                    tracing::info!(
                        "OAuth authentication requires Claude Code compatibility mode (auto-enabled)"
                    );

                    return Ok(Box::new(anthropic::AnthropicClient::with_token_manager(
                        token_manager,
                        config.llm.model.clone(),
                        config.llm.max_tokens,
                        true, // Always enable for OAuth - it's required
                    )));
                }

                // For API key tokens, use the legacy path
                if !stored_token.is_expired() {
                    tracing::info!("Using stored API key authentication for Anthropic");
                    return Ok(Box::new(anthropic::AnthropicClient::from_token(
                        &stored_token,
                        config.llm.model.clone(),
                        config.llm.max_tokens,
                        config.llm.claude_code_oauth_compat,
                    )));
                }
            }

            // Fall back to configured API key or environment variable
            let api_key = config.get_auth_token()
                .context("Anthropic API key not set. Use 'safe-coder login anthropic' or set ANTHROPIC_API_KEY")?;
            Ok(Box::new(anthropic::AnthropicClient::new(
                api_key,
                config.llm.model.clone(),
                config.llm.max_tokens,
            )))
        }
        LlmProvider::OpenAI => {
            let api_key = config
                .get_auth_token()
                .context("OpenAI API key not set. Set OPENAI_API_KEY or configure API key")?;
            Ok(Box::new(openai::OpenAiClient::new(
                api_key,
                config.llm.model.clone(),
                config.llm.max_tokens,
                config.llm.base_url.clone(),
            )))
        }
        LlmProvider::Ollama => {
            tracing::info!("🦙 Using Ollama (local LLM)");
            Ok(Box::new(ollama::OllamaClient::new(
                config.llm.base_url.clone(),
                config.llm.model.clone(),
                config.llm.max_tokens,
            )))
        }
        LlmProvider::GitHubCopilot => {
            // For GitHub Copilot, we need to exchange the GitHub token for a Copilot token
            let github_token = config.get_auth_token()
                .context("GitHub Copilot token not set. Use 'safe-coder login github-copilot' or set GITHUB_COPILOT_TOKEN")?;

            // Get the Copilot-specific token
            let copilot_token = copilot::get_copilot_token(&github_token).await
                .context("Failed to get GitHub Copilot token. Make sure you have an active Copilot subscription")?;

            Ok(Box::new(copilot::CopilotClient::new(
                copilot_token,
                config.llm.model.clone(),
                config.llm.max_tokens,
            )))
        }
    }
}

impl Message {
    pub fn user(text: String) -> Self {
        Self {
            role: Role::User,
            content: vec![ContentBlock::Text { text }],
        }
    }

    pub fn assistant(content: Vec<ContentBlock>) -> Self {
        Self {
            role: Role::Assistant,
            content,
        }
    }
}
