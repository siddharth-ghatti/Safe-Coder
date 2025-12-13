use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::config::{LlmConfig, LlmProvider};

pub mod anthropic;
pub mod openai;
pub mod ollama;
pub mod copilot;

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
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn send_message(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<Message>;
}

pub async fn create_client(config: &crate::config::Config) -> Result<Box<dyn LlmClient>> {
    match config.llm.provider {
        LlmProvider::Anthropic => {
            // Check if we have a stored token (could be OAuth or API key)
            if let Some(stored_token) = config.get_stored_token() {
                if !stored_token.is_expired() {
                    tracing::info!("Using stored {} authentication for Anthropic",
                        if stored_token.is_oauth() { "OAuth" } else { "API key" });
                    return Ok(Box::new(anthropic::AnthropicClient::from_token(
                        &stored_token,
                        config.llm.model.clone(),
                        config.llm.max_tokens,
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
            let api_key = config.get_auth_token()
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
