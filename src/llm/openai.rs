use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{ContentBlock, LlmClient, Message, Role, ToolDefinition};

pub struct OpenAiClient {
    api_key: String,
    model: String,
    max_tokens: usize,
    base_url: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenAiTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenAiFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct OpenAiTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiToolFunction,
}

#[derive(Debug, Serialize)]
struct OpenAiToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiResponseMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseMessage {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAiToolCall>,
}

impl OpenAiClient {
    pub fn new(
        api_key: String,
        model: String,
        max_tokens: usize,
        base_url: Option<String>,
    ) -> Self {
        Self {
            api_key,
            model,
            max_tokens,
            base_url: base_url.unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            client: reqwest::Client::new(),
        }
    }

    fn convert_messages(&self, messages: &[Message]) -> Vec<OpenAiMessage> {
        messages
            .iter()
            .flat_map(|msg| {
                let role = match msg.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                };

                msg.content.iter().map(move |block| match block {
                    ContentBlock::Text { text } => OpenAiMessage {
                        role: role.to_string(),
                        content: Some(text.clone()),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    },
                    ContentBlock::ToolUse { id, name, input } => OpenAiMessage {
                        role: "assistant".to_string(),
                        content: None,
                        tool_calls: Some(vec![OpenAiToolCall {
                            id: id.clone(),
                            call_type: "function".to_string(),
                            function: OpenAiFunction {
                                name: name.clone(),
                                arguments: serde_json::to_string(input).unwrap_or_default(),
                            },
                        }]),
                        tool_call_id: None,
                        name: None,
                    },
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                    } => OpenAiMessage {
                        role: "tool".to_string(),
                        content: Some(content.clone()),
                        tool_calls: None,
                        tool_call_id: Some(tool_use_id.clone()),
                        name: None,
                    },
                })
            })
            .collect()
    }
}

#[async_trait]
impl LlmClient for OpenAiClient {
    async fn send_message_with_system(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system_prompt: Option<&str>,
    ) -> Result<Message> {
        // Build messages, prepending system prompt if provided
        let mut openai_messages = Vec::new();

        if let Some(system) = system_prompt {
            openai_messages.push(OpenAiMessage {
                role: "system".to_string(),
                content: Some(system.to_string()),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        }

        openai_messages.extend(self.convert_messages(messages));

        let openai_tools: Vec<OpenAiTool> = tools
            .iter()
            .map(|tool| OpenAiTool {
                tool_type: "function".to_string(),
                function: OpenAiToolFunction {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    parameters: tool.input_schema.clone(),
                },
            })
            .collect();

        let request = OpenAiRequest {
            model: self.model.clone(),
            messages: openai_messages,
            tools: openai_tools,
            max_tokens: Some(self.max_tokens),
        };

        let url = format!("{}/chat/completions", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to OpenAI")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("OpenAI API error ({}): {}", status, error_text);
        }

        let openai_response: OpenAiResponse = response
            .json()
            .await
            .context("Failed to parse OpenAI response")?;

        let choice = openai_response
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("No response from OpenAI"))?;

        let mut content_blocks = Vec::new();

        // Add text content if present
        if let Some(text) = &choice.message.content {
            if !text.is_empty() {
                content_blocks.push(ContentBlock::Text { text: text.clone() });
            }
        }

        // Add tool calls if present
        for tool_call in &choice.message.tool_calls {
            let input: serde_json::Value = serde_json::from_str(&tool_call.function.arguments)
                .unwrap_or(serde_json::Value::Null);

            content_blocks.push(ContentBlock::ToolUse {
                id: tool_call.id.clone(),
                name: tool_call.function.name.clone(),
                input,
            });
        }

        // If no content blocks, add empty text
        if content_blocks.is_empty() {
            content_blocks.push(ContentBlock::Text {
                text: String::new(),
            });
        }

        Ok(Message {
            role: Role::Assistant,
            content: content_blocks,
        })
    }
}
