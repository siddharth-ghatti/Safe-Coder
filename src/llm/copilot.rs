use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{ContentBlock, LlmClient, LlmResponse, Message, Role, ToolDefinition};

// Helper function to get Copilot token from GitHub token
pub async fn get_copilot_token(github_token: &str) -> Result<String> {
    let client = reqwest::Client::new();

    // GitHub Copilot token endpoint
    let response = client
        .get("https://api.github.com/copilot_internal/v2/token")
        .header("Authorization", format!("token {}", github_token))
        .header("Accept", "application/json")
        .send()
        .await
        .context("Failed to get Copilot token")?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await?;
        anyhow::bail!("Failed to get Copilot token ({}): {}", status, text);
    }

    #[derive(Deserialize)]
    struct CopilotTokenResponse {
        token: String,
    }

    let copilot_response: CopilotTokenResponse = response
        .json()
        .await
        .context("Failed to parse Copilot token response")?;

    Ok(copilot_response.token)
}

pub struct CopilotClient {
    api_key: String,
    model: String,
    max_tokens: usize,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct CopilotRequest {
    model: String,
    messages: Vec<CopilotMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<CopilotTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CopilotMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<CopilotToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CopilotToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: CopilotFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct CopilotFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct CopilotTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: CopilotToolFunction,
}

#[derive(Debug, Serialize)]
struct CopilotToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct CopilotResponse {
    choices: Vec<CopilotChoice>,
}

#[derive(Debug, Deserialize)]
struct CopilotChoice {
    message: CopilotResponseMessage,
}

#[derive(Debug, Deserialize)]
struct CopilotResponseMessage {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<CopilotToolCall>,
}

impl CopilotClient {
    pub fn new(api_key: String, model: String, max_tokens: usize) -> Self {
        Self {
            api_key,
            model,
            max_tokens,
            client: reqwest::Client::new(),
        }
    }

    fn convert_messages(&self, messages: &[Message]) -> Vec<CopilotMessage> {
        messages
            .iter()
            .flat_map(|msg| {
                let role = match msg.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                };

                msg.content.iter().map(move |block| match block {
                    ContentBlock::Text { text } => CopilotMessage {
                        role: role.to_string(),
                        content: Some(text.clone()),
                        tool_calls: None,
                        tool_call_id: None,
                    },
                    ContentBlock::ToolUse { id, name, input } => CopilotMessage {
                        role: "assistant".to_string(),
                        content: None,
                        tool_calls: Some(vec![CopilotToolCall {
                            id: id.clone(),
                            call_type: "function".to_string(),
                            function: CopilotFunction {
                                name: name.clone(),
                                arguments: serde_json::to_string(input).unwrap_or_default(),
                            },
                        }]),
                        tool_call_id: None,
                    },
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                    } => CopilotMessage {
                        role: "tool".to_string(),
                        content: Some(content.clone()),
                        tool_calls: None,
                        tool_call_id: Some(tool_use_id.clone()),
                    },
                })
            })
            .collect()
    }
}

#[async_trait]
impl LlmClient for CopilotClient {
    async fn send_message_with_system(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system_prompt: Option<&str>,
    ) -> Result<LlmResponse> {
        // Build messages, prepending system prompt if provided
        let mut copilot_messages = Vec::new();

        if let Some(system) = system_prompt {
            copilot_messages.push(CopilotMessage {
                role: "system".to_string(),
                content: Some(system.to_string()),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        copilot_messages.extend(self.convert_messages(messages));

        let copilot_tools: Vec<CopilotTool> = tools
            .iter()
            .map(|tool| CopilotTool {
                tool_type: "function".to_string(),
                function: CopilotToolFunction {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    parameters: tool.input_schema.clone(),
                },
            })
            .collect();

        let request = CopilotRequest {
            model: self.model.clone(),
            messages: copilot_messages,
            tools: copilot_tools,
            max_tokens: Some(self.max_tokens),
        };

        // GitHub Copilot uses the same endpoint structure as OpenAI
        let url = "https://api.githubcopilot.com/chat/completions";

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("Failed to send request to GitHub Copilot")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("GitHub Copilot API error ({}): {}", status, error_text);
        }

        let copilot_response: CopilotResponse = response
            .json()
            .await
            .context("Failed to parse GitHub Copilot response")?;

        let choice = copilot_response
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("No response from GitHub Copilot"))?;

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

        Ok(LlmResponse {
            message: Message {
                role: Role::Assistant,
                content: content_blocks,
            },
            usage: None, // Copilot client doesn't track usage yet
        })
    }
}
