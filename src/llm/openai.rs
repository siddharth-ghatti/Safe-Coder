use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{ContentBlock, LlmClient, Message, Role, ToolDefinition};

pub struct OpenAiClient {
    api_key: String,
    model: String,
    max_tokens: usize,
    client: reqwest::Client,
}

impl OpenAiClient {
    pub fn new(api_key: String, model: String, max_tokens: usize) -> Self {
        Self {
            api_key,
            model,
            max_tokens,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn send_message(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
    ) -> Result<Message> {
        // Placeholder - implement OpenAI API integration here
        anyhow::bail!("OpenAI client not yet implemented")
    }
}
