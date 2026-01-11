use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::openai_compat::{self, OpenAiCompatMessage};
use super::{ContentBlock, LlmClient, LlmResponse, Message, Role, TokenUsage, ToolDefinition};

/// Information about a single Copilot model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotModel {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub model_picker_enabled: Option<bool>,
    #[serde(default)]
    pub preview: Option<bool>,
}

/// Response from the models endpoint
#[derive(Debug, Deserialize)]
struct ModelsResponse {
    data: Vec<CopilotModel>,
}

/// Fetch available models from GitHub Copilot
pub async fn get_copilot_models(copilot_token: &str) -> Result<Vec<CopilotModel>> {
    let client = reqwest::Client::new();

    let response = client
        .get("https://api.githubcopilot.com/models")
        .header("Authorization", format!("Bearer {}", copilot_token))
        .header("Content-Type", "application/json")
        .header("Copilot-Integration-Id", "vscode-chat")
        .send()
        .await
        .context("Failed to fetch Copilot models")?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await?;
        anyhow::bail!("Failed to fetch Copilot models ({}): {}", status, text);
    }

    let models_response: ModelsResponse = response
        .json()
        .await
        .context("Failed to parse Copilot models response")?;

    Ok(models_response.data)
}

// Helper function to get Copilot token from GitHub token
pub async fn get_copilot_token(github_token: &str) -> Result<String> {
    tracing::info!("Getting Copilot token from GitHub token ({}...)", &github_token[..github_token.len().min(10)]);
    let client = reqwest::Client::new();

    // GitHub Copilot token endpoint
    let response = match client
        .get("https://api.github.com/copilot_internal/v2/token")
        .header("Authorization", format!("token {}", github_token))
        .header("Accept", "application/json")
        .header("User-Agent", "safe-coder/1.0")
        .header("Editor-Version", "vscode/1.85.0")
        .header("Editor-Plugin-Version", "copilot-chat/0.12.0")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            anyhow::bail!("HTTP request failed: {}", e);
        }
    };

    let status = response.status();
    tracing::info!("Copilot token response status: {}", status);

    if !status.is_success() {
        let text = response.text().await.unwrap_or_default();
        tracing::error!("Failed to get Copilot token ({}): {}", status, text);
        anyhow::bail!("API returned error ({}): {}", status, text);
    }

    let response_text = response.text().await.context("Failed to read response body")?;

    #[derive(Deserialize)]
    struct CopilotTokenResponse {
        token: String,
    }

    let copilot_response: CopilotTokenResponse = serde_json::from_str(&response_text)
        .context(format!("Failed to parse response: {}", &response_text[..response_text.len().min(200)]))?;

    tracing::info!("Successfully obtained Copilot token");
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

/// Content can be either a simple string or an array of content parts (for multimodal)
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum CopilotContent {
    Text(String),
    Parts(Vec<CopilotContentPart>),
}

/// Content part for multimodal messages
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum CopilotContentPart {
    Text { text: String },
    ImageUrl { image_url: CopilotImageUrlData },
}

#[derive(Debug, Serialize, Deserialize)]
struct CopilotImageUrlData {
    url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CopilotMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<CopilotContent>,
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

/// Usage information from Copilot response (OpenAI-compatible)
#[derive(Debug, Deserialize)]
struct CopilotUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
}

#[derive(Debug, Deserialize)]
struct CopilotResponse {
    choices: Vec<CopilotChoice>,
    #[serde(default)]
    usage: Option<CopilotUsage>,
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

    /// Convert and validate messages using shared OpenAI-compatible logic
    fn prepare_messages(&self, messages: &[Message]) -> Vec<CopilotMessage> {
        // Use shared conversion logic
        let compat_messages = openai_compat::convert_messages(messages);
        // Validate tool call/result pairs
        let validated = openai_compat::validate_tool_pairs(compat_messages);
        // Convert to Copilot-specific format
        validated.into_iter().map(Self::from_compat_message).collect()
    }

    /// Convert from shared format to Copilot-specific format
    fn from_compat_message(msg: OpenAiCompatMessage) -> CopilotMessage {
        // Handle multimodal content (images) vs simple text
        let content = if let Some(parts) = msg.content_parts {
            // Multimodal: convert to content parts array
            let copilot_parts: Vec<CopilotContentPart> = parts
                .into_iter()
                .map(|part| match part {
                    openai_compat::OpenAiContentPart::Text { text } => {
                        CopilotContentPart::Text { text }
                    }
                    openai_compat::OpenAiContentPart::ImageUrl { url } => {
                        CopilotContentPart::ImageUrl {
                            image_url: CopilotImageUrlData { url },
                        }
                    }
                })
                .collect();
            Some(CopilotContent::Parts(copilot_parts))
        } else {
            // Simple text content
            msg.content.map(CopilotContent::Text)
        };

        CopilotMessage {
            role: msg.role,
            content,
            tool_calls: msg.tool_calls.map(|calls| {
                calls.into_iter().map(|tc| CopilotToolCall {
                    id: tc.id,
                    call_type: tc.call_type,
                    function: CopilotFunction {
                        name: tc.function_name,
                        arguments: tc.function_arguments,
                    },
                }).collect()
            }),
            tool_call_id: msg.tool_call_id,
        }
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
                content: Some(CopilotContent::Text(system.to_string())),
                tool_calls: None,
                tool_call_id: None,
            });
        }

        copilot_messages.extend(self.prepare_messages(messages));

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

        // Check if any messages contain images (need vision header)
        let has_images = messages.iter().any(|msg| {
            msg.content.iter().any(|block| matches!(block, ContentBlock::Image { .. }))
        });

        // GitHub Copilot uses the same endpoint structure as OpenAI
        let url = "https://api.githubcopilot.com/chat/completions";

        let mut req_builder = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .header("Editor-Version", "vscode/1.85.0")
            .header("Editor-Plugin-Version", "copilot-chat/0.12.0")
            .header("Copilot-Integration-Id", "vscode-chat")
            .header("User-Agent", "GitHubCopilotChat/0.12.0");

        // Add vision header if images are present
        if has_images {
            tracing::info!("Adding Copilot-Vision-Request header for image request");
            req_builder = req_builder.header("Copilot-Vision-Request", "true");
        }

        let response = req_builder
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

        // Extract token usage (Copilot uses OpenAI-compatible format)
        let usage = copilot_response
            .usage
            .map(|u| TokenUsage::new(u.prompt_tokens, u.completion_tokens));

        Ok(LlmResponse {
            message: Message {
                role: Role::Assistant,
                content: content_blocks,
            },
            usage,
        })
    }
}
