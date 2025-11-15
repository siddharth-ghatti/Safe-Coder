use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::config::{LlmConfig, LlmProvider};

pub mod anthropic;
pub mod openai;
pub mod ollama;

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

pub fn create_client(config: &LlmConfig) -> Result<Box<dyn LlmClient>> {
    match config.provider {
        LlmProvider::Anthropic => {
            let api_key = config.api_key.as_ref()
                .context("Anthropic API key not set")?;
            Ok(Box::new(anthropic::AnthropicClient::new(
                api_key.clone(),
                config.model.clone(),
                config.max_tokens,
            )))
        }
        LlmProvider::OpenAI => {
            let api_key = config.api_key.as_ref()
                .context("OpenAI API key not set")?;
            Ok(Box::new(openai::OpenAiClient::new(
                api_key.clone(),
                config.model.clone(),
                config.max_tokens,
            )))
        }
        LlmProvider::Ollama => {
            tracing::info!("🦙 Using Ollama (local LLM)");
            Ok(Box::new(ollama::OllamaClient::new(
                config.base_url.clone(),
                config.model.clone(),
                config.max_tokens,
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
