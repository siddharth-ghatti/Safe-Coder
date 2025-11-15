use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{ContentBlock, LlmClient, Message, Role, ToolDefinition};

pub struct AnthropicClient {
    api_key: String,
    model: String,
    max_tokens: usize,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: usize,
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
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String },
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
    pub fn new(api_key: String, model: String, max_tokens: usize) -> Self {
        Self {
            api_key,
            model,
            max_tokens,
            client: reqwest::Client::new(),
        }
    }

    fn convert_message_to_anthropic(msg: &Message) -> AnthropicMessage {
        AnthropicMessage {
            role: match msg.role {
                Role::User => "user".to_string(),
                Role::Assistant => "assistant".to_string(),
            },
            content: msg.content.iter().map(|c| match c {
                ContentBlock::Text { text } => AnthropicContent::Text { text: text.clone() },
                ContentBlock::ToolUse { id, name, input } => AnthropicContent::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                },
                ContentBlock::ToolResult { tool_use_id, content } => AnthropicContent::ToolResult {
                    tool_use_id: tool_use_id.clone(),
                    content: content.clone(),
                },
            }).collect(),
        }
    }

    fn convert_anthropic_to_message(content: Vec<AnthropicContent>) -> Message {
        Message {
            role: Role::Assistant,
            content: content.into_iter().map(|c| match c {
                AnthropicContent::Text { text } => ContentBlock::Text { text },
                AnthropicContent::ToolUse { id, name, input } => ContentBlock::ToolUse {
                    id,
                    name,
                    input,
                },
                AnthropicContent::ToolResult { tool_use_id, content } => ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                },
            }).collect(),
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
        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            messages: messages.iter()
                .map(Self::convert_message_to_anthropic)
                .collect(),
            tools: tools.iter().map(|t| AnthropicTool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            }).collect(),
        };

        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Anthropic")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            anyhow::bail!("Anthropic API error ({}): {}", status, text);
        }

        let anthropic_response: AnthropicResponse = response.json()
            .await
            .context("Failed to parse Anthropic response")?;

        Ok(Self::convert_anthropic_to_message(anthropic_response.content))
    }
}
