//! Generic OpenAI-compatible API client
//!
//! This client works with any server that implements the OpenAI chat completions API,
//! including:
//! - vLLM
//! - LocalAI
//! - LiteLLM
//! - text-generation-inference (TGI)
//! - LM Studio
//! - Ollama (with OpenAI compatibility mode)
//! - Any other OpenAI-compatible API

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::openai_compat::{self, OpenAiCompatMessage};
use super::{ContentBlock, LlmClient, LlmResponse, Message, Role, TokenUsage, ToolDefinition};

/// A client for any OpenAI-compatible API endpoint
pub struct GenericOpenAiClient {
    /// Optional API key (some self-hosted servers don't require auth)
    api_key: Option<String>,
    /// The model name to use
    model: String,
    /// Maximum tokens to generate
    max_tokens: usize,
    /// Base URL for the API (required, e.g., "http://localhost:8000/v1")
    base_url: String,
    /// HTTP client
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

/// Content can be either a simple string or an array of content parts (for multimodal)
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum OpenAiContent {
    Text(String),
    Parts(Vec<OpenAiContentPart>),
}

/// Content part for multimodal messages
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OpenAiContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrlData },
}

#[derive(Debug, Serialize, Deserialize)]
struct ImageUrlData {
    url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<OpenAiContent>,
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

/// Prompt token details including cache information
#[derive(Debug, Deserialize, Default)]
struct PromptTokensDetails {
    /// Number of tokens that were served from cache
    #[serde(default)]
    cached_tokens: usize,
}

/// Usage information from OpenAI response
#[derive(Debug, Deserialize)]
struct OpenAiUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    #[serde(default)]
    prompt_tokens_details: Option<PromptTokensDetails>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    #[serde(default)]
    usage: Option<OpenAiUsage>,
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

impl GenericOpenAiClient {
    /// Create a new generic OpenAI-compatible client
    ///
    /// # Arguments
    /// * `base_url` - The base URL for the API (e.g., "http://localhost:8000/v1")
    /// * `model` - The model name to use
    /// * `max_tokens` - Maximum tokens to generate
    /// * `api_key` - Optional API key (some servers don't require auth)
    pub fn new(
        base_url: String,
        model: String,
        max_tokens: usize,
        api_key: Option<String>,
    ) -> Self {
        // Create client with 120 second timeout to prevent hanging
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        // Normalize base URL - remove trailing slash if present
        let base_url = base_url.trim_end_matches('/').to_string();

        Self {
            api_key,
            model,
            max_tokens,
            base_url,
            client,
        }
    }

    /// Convert and validate messages using shared OpenAI-compatible logic
    fn prepare_messages(&self, messages: &[Message]) -> Vec<OpenAiMessage> {
        // Use shared conversion logic
        let compat_messages = openai_compat::convert_messages(messages);
        // Validate tool call/result pairs
        let validated = openai_compat::validate_tool_pairs(compat_messages);
        // Convert to OpenAI-specific format
        validated
            .into_iter()
            .map(Self::from_compat_message)
            .collect()
    }

    /// Convert from shared format to OpenAI-specific format
    fn from_compat_message(msg: OpenAiCompatMessage) -> OpenAiMessage {
        // Handle multimodal content (images) vs simple text
        let content = if let Some(parts) = msg.content_parts {
            // Multimodal: convert to content parts array
            let openai_parts: Vec<OpenAiContentPart> = parts
                .into_iter()
                .map(|part| match part {
                    openai_compat::OpenAiContentPart::Text { text } => {
                        OpenAiContentPart::Text { text }
                    }
                    openai_compat::OpenAiContentPart::ImageUrl { url } => {
                        OpenAiContentPart::ImageUrl {
                            image_url: ImageUrlData { url },
                        }
                    }
                })
                .collect();
            Some(OpenAiContent::Parts(openai_parts))
        } else {
            // Simple text content
            msg.content.map(OpenAiContent::Text)
        };

        OpenAiMessage {
            role: msg.role,
            content,
            tool_calls: msg.tool_calls.map(|calls| {
                calls
                    .into_iter()
                    .map(|tc| OpenAiToolCall {
                        id: tc.id,
                        call_type: tc.call_type,
                        function: OpenAiFunction {
                            name: tc.function_name,
                            arguments: tc.function_arguments,
                        },
                    })
                    .collect()
            }),
            tool_call_id: msg.tool_call_id,
            name: None,
        }
    }
}

#[async_trait]
impl LlmClient for GenericOpenAiClient {
    async fn send_message_with_system(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system_prompt: Option<&str>,
    ) -> Result<LlmResponse> {
        // Build messages, prepending system prompt if provided
        let mut openai_messages = Vec::new();

        if let Some(system) = system_prompt {
            openai_messages.push(OpenAiMessage {
                role: "system".to_string(),
                content: Some(OpenAiContent::Text(system.to_string())),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        }

        openai_messages.extend(self.prepare_messages(messages));

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

        // Build the request
        let mut req = self
            .client
            .post(&url)
            .header("Content-Type", "application/json");

        // Add authorization header only if API key is provided
        if let Some(ref api_key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        let response = req
            .json(&request)
            .send()
            .await
            .with_context(|| format!("Failed to send request to OpenAI-compatible API at {}", url))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!(
                "OpenAI-compatible API error ({}): {}",
                status,
                error_text
            );
        }

        let openai_response: OpenAiResponse = response
            .json()
            .await
            .context("Failed to parse OpenAI-compatible API response")?;

        let choice = openai_response
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("No response from OpenAI-compatible API"))?;

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

        // Extract token usage including cache information
        let usage = openai_response.usage.map(|u| {
            let cached_tokens = u
                .prompt_tokens_details
                .map(|d| d.cached_tokens)
                .unwrap_or(0);

            // Log cache stats if present
            if cached_tokens > 0 {
                tracing::debug!(
                    "OpenAI-compatible API cache hit: {} tokens read from cache",
                    cached_tokens
                );
            }

            TokenUsage::with_cache(
                u.prompt_tokens,
                u.completion_tokens,
                None, // Most servers don't have cache creation tokens
                if cached_tokens > 0 {
                    Some(cached_tokens)
                } else {
                    None
                },
            )
        });

        Ok(LlmResponse {
            message: Message {
                role: Role::Assistant,
                content: content_blocks,
            },
            usage,
        })
    }
}
