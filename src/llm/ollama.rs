use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{ContentBlock, LlmClient, LlmResponse, Message, Role, ToolDefinition};

pub struct OllamaClient {
    base_url: String,
    model: String,
    max_tokens: usize,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OllamaTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
}

#[derive(Debug, Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OllamaToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct OllamaToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OllamaFunction,
}

#[derive(Debug, Serialize)]
struct OllamaFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct OllamaTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OllamaToolFunction,
}

#[derive(Debug, Serialize)]
struct OllamaToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    choices: Vec<OllamaChoice>,
}

#[derive(Debug, Deserialize)]
struct OllamaChoice {
    message: OllamaResponseMessage,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OllamaResponseToolCall>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseToolCall {
    id: String,
    function: OllamaResponseFunction,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseFunction {
    name: String,
    arguments: String,
}

impl OllamaClient {
    pub fn new(base_url: Option<String>, model: String, max_tokens: usize) -> Self {
        Self {
            base_url: base_url.unwrap_or_else(|| "http://localhost:11434".to_string()),
            model,
            max_tokens,
            client: reqwest::Client::new(),
        }
    }

    fn convert_messages(&self, messages: &[Message]) -> Vec<OllamaMessage> {
        messages
            .iter()
            .flat_map(|msg| {
                let role = match msg.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                };

                msg.content.iter().map(move |block| match block {
                    ContentBlock::Text { text } => OllamaMessage {
                        role: role.to_string(),
                        content: text.clone(),
                        tool_calls: None,
                        tool_call_id: None,
                    },
                    ContentBlock::ToolUse { id, name, input } => OllamaMessage {
                        role: "assistant".to_string(),
                        content: String::new(),
                        tool_calls: Some(vec![OllamaToolCall {
                            id: id.clone(),
                            call_type: "function".to_string(),
                            function: OllamaFunction {
                                name: name.clone(),
                                arguments: serde_json::to_string(input).unwrap_or_default(),
                            },
                        }]),
                        tool_call_id: None,
                    },
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                    } => OllamaMessage {
                        role: "tool".to_string(),
                        content: content.clone(),
                        tool_calls: None,
                        tool_call_id: Some(tool_use_id.clone()),
                    },
                })
            })
            .collect()
    }
}

#[async_trait]
impl LlmClient for OllamaClient {
    async fn send_message_with_system(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system_prompt: Option<&str>,
    ) -> Result<LlmResponse> {
        // Build messages, prepending system prompt if provided
        let mut ollama_messages = Vec::new();

        if let Some(system) = system_prompt {
            ollama_messages.push(OllamaMessage {
                role: "system".to_string(),
                content: system.to_string(),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        ollama_messages.extend(self.convert_messages(messages));

        let ollama_tools = if !tools.is_empty() {
            Some(
                tools
                    .iter()
                    .map(|tool| OllamaTool {
                        tool_type: "function".to_string(),
                        function: OllamaToolFunction {
                            name: tool.name.clone(),
                            description: tool.description.clone(),
                            parameters: tool.input_schema.clone(),
                        },
                    })
                    .collect(),
            )
        } else {
            None
        };

        let request = OllamaRequest {
            model: self.model.clone(),
            messages: ollama_messages,
            tools: ollama_tools,
            max_tokens: Some(self.max_tokens),
        };

        let url = format!("{}/v1/chat/completions", self.base_url);

        let response = self.client.post(&url).json(&request).send().await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("Ollama API error ({}): {}", status, error_text);
        }

        let ollama_response: OllamaResponse = response.json().await?;

        let choice = ollama_response
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("No response from Ollama"))?;

        let mut content_blocks = Vec::new();

        // Add text content if present
        if let Some(text) = &choice.message.content {
            if !text.is_empty() {
                content_blocks.push(ContentBlock::Text { text: text.clone() });
            }
        }

        // Add tool calls if present
        for tool_call in &choice.message.tool_calls {
            let input: serde_json::Value =
                serde_json::from_str(&tool_call.function.arguments).unwrap_or_default();

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

        Ok(LlmResponse {
            message: Message {
                role: Role::Assistant,
                content: content_blocks,
            },
            usage: None, // Ollama client doesn't track usage yet
        })
    }
}
