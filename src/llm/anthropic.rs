use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::{ContentBlock, LlmClient, Message, Role, ToolDefinition};
use crate::auth::anthropic::get_oauth_beta_headers;
use crate::auth::{StoredToken, TokenManager};

/// Authentication type for the Anthropic client
#[derive(Debug, Clone)]
pub enum AuthType {
    /// API key authentication (x-api-key header)
    ApiKey(String),
    /// OAuth Bearer token with automatic refresh via TokenManager
    OAuth {
        /// Token manager handles refresh automatically
        token_manager: Arc<TokenManager>,
    },
    /// Legacy OAuth without token manager (for backwards compatibility)
    OAuthLegacy {
        access_token: String,
        refresh_token: String,
    },
}

/// The system prompt required for Claude Code OAuth compatibility
/// This makes OAuth tokens work with the API by identifying as Claude Code
const CLAUDE_CODE_SYSTEM_PROMPT: &str = "You are Claude Code, Anthropic's official CLI for Claude.";

pub struct AnthropicClient {
    auth: AuthType,
    model: String,
    max_tokens: usize,
    client: reqwest::Client,
    /// Enable Claude Code OAuth compatibility mode (injects system prompt)
    claude_code_compat: bool,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<AnthropicTool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: Vec<AnthropicContent>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContent {
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

#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    stop_reason: Option<String>,
}

impl AnthropicClient {
    /// Create a new Anthropic client with API key authentication
    pub fn new(api_key: String, model: String, max_tokens: usize) -> Self {
        Self {
            auth: AuthType::ApiKey(api_key),
            model,
            max_tokens,
            client: reqwest::Client::new(),
            claude_code_compat: false,
        }
    }

    /// Create a new Anthropic client from a stored token (legacy, no auto-refresh)
    pub fn from_token(
        token: &StoredToken,
        model: String,
        max_tokens: usize,
        claude_code_compat: bool,
    ) -> Self {
        let auth = match token {
            StoredToken::Api { key } => AuthType::ApiKey(key.clone()),
            StoredToken::OAuth {
                access_token,
                refresh_token,
                ..
            } => AuthType::OAuthLegacy {
                access_token: access_token.clone(),
                refresh_token: refresh_token.clone(),
            },
            StoredToken::Device { access_token, .. } => AuthType::ApiKey(access_token.clone()),
        };

        Self {
            auth,
            model,
            max_tokens,
            client: reqwest::Client::new(),
            claude_code_compat,
        }
    }

    /// Create a new Anthropic client with a token manager for automatic refresh
    pub fn with_token_manager(
        token_manager: Arc<TokenManager>,
        model: String,
        max_tokens: usize,
        claude_code_compat: bool,
    ) -> Self {
        Self {
            auth: AuthType::OAuth { token_manager },
            model,
            claude_code_compat,
            max_tokens,
            client: reqwest::Client::new(),
        }
    }

    /// Check if using OAuth authentication
    pub fn is_oauth(&self) -> bool {
        matches!(
            self.auth,
            AuthType::OAuth { .. } | AuthType::OAuthLegacy { .. }
        )
    }

    /// Get the current access token (refreshing if needed for managed OAuth)
    async fn get_access_token(&self) -> Result<String> {
        match &self.auth {
            AuthType::ApiKey(key) => Ok(key.clone()),
            AuthType::OAuth { token_manager } => {
                // This will automatically refresh if needed
                token_manager.get_valid_token().await
            }
            AuthType::OAuthLegacy { access_token, .. } => Ok(access_token.clone()),
        }
    }

    fn convert_message_to_anthropic(msg: &Message) -> AnthropicMessage {
        AnthropicMessage {
            role: match msg.role {
                Role::User => "user".to_string(),
                Role::Assistant => "assistant".to_string(),
            },
            content: msg
                .content
                .iter()
                .map(|c| match c {
                    ContentBlock::Text { text } => AnthropicContent::Text { text: text.clone() },
                    ContentBlock::ToolUse { id, name, input } => AnthropicContent::ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    },
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                    } => AnthropicContent::ToolResult {
                        tool_use_id: tool_use_id.clone(),
                        content: content.clone(),
                    },
                })
                .collect(),
        }
    }

    fn convert_anthropic_to_message(content: Vec<AnthropicContent>) -> Message {
        Message {
            role: Role::Assistant,
            content: content
                .into_iter()
                .map(|c| match c {
                    AnthropicContent::Text { text } => ContentBlock::Text { text },
                    AnthropicContent::ToolUse { id, name, input } => {
                        ContentBlock::ToolUse { id, name, input }
                    }
                    AnthropicContent::ToolResult {
                        tool_use_id,
                        content,
                    } => ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                    },
                })
                .collect(),
        }
    }
}

#[async_trait]
impl LlmClient for AnthropicClient {
    async fn send_message(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<Message> {
        // Determine if we should inject the Claude Code system prompt
        // This is needed for OAuth tokens to work with the API
        let system_prompt = if self.claude_code_compat && self.is_oauth() {
            Some(CLAUDE_CODE_SYSTEM_PROMPT.to_string())
        } else {
            None
        };

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            system: system_prompt,
            messages: messages
                .iter()
                .map(Self::convert_message_to_anthropic)
                .collect(),
            tools: tools
                .iter()
                .map(|t| AnthropicTool {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    input_schema: t.input_schema.clone(),
                })
                .collect(),
        };

        // Build the request with appropriate auth headers
        let mut req_builder = self
            .client
            .post("https://api.anthropic.com/v1/messages")
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json");

        // Add authentication headers based on auth type
        let is_oauth = self.is_oauth();
        if is_oauth {
            // Get access token (may trigger automatic refresh)
            let access_token = self
                .get_access_token()
                .await
                .context("Failed to get access token")?;

            // OAuth uses Bearer token and special beta headers
            req_builder = req_builder
                .header("Authorization", format!("Bearer {}", access_token))
                .header("anthropic-beta", get_oauth_beta_headers());
        } else {
            // API key auth
            let api_key = self.get_access_token().await?;
            req_builder = req_builder.header("x-api-key", api_key);
        }

        let response = req_builder
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Anthropic")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            anyhow::bail!("Anthropic API error ({}): {}", status, text);
        }

        let anthropic_response: AnthropicResponse = response
            .json()
            .await
            .context("Failed to parse Anthropic response")?;

        Ok(Self::convert_anthropic_to_message(
            anthropic_response.content,
        ))
    }
}
