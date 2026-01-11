//! OpenRouter LLM provider
//!
//! OpenRouter provides access to 75+ models through a unified API.
//! It uses an OpenAI-compatible API with additional headers.
//!
//! Supported models include:
//! - anthropic/claude-3.5-sonnet
//! - openai/gpt-4o
//! - google/gemini-pro-1.5
//! - meta-llama/llama-3.1-405b-instruct
//! - mistralai/mixtral-8x22b-instruct
//! - And many more...
//!
//! See https://openrouter.ai/models for the full list.

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::openai_compat::{self, OpenAiCompatMessage};
use super::{ContentBlock, LlmClient, LlmResponse, Message, Role, TokenUsage, ToolDefinition};

const OPENROUTER_API_URL: &str = "https://openrouter.ai/api/v1";

pub struct OpenRouterClient {
    api_key: String,
    model: String,
    max_tokens: usize,
    client: reqwest::Client,
    /// Optional site URL for OpenRouter rankings
    site_url: Option<String>,
    /// Optional site name for OpenRouter rankings
    site_name: Option<String>,
}

#[derive(Debug, Serialize)]
struct OpenRouterRequest {
    model: String,
    messages: Vec<OpenRouterMessage>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tools: Vec<OpenRouterTool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    /// OpenRouter-specific: allow fallback to similar models if primary is unavailable
    #[serde(skip_serializing_if = "Option::is_none")]
    route: Option<String>,
}

/// Content can be either a simple string or an array of content parts (for multimodal)
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum OpenRouterContent {
    Text(String),
    Parts(Vec<OpenRouterContentPart>),
}

/// Content part for multimodal messages
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OpenRouterContentPart {
    Text { text: String },
    ImageUrl { image_url: OpenRouterImageUrlData },
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenRouterImageUrlData {
    url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenRouterMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<OpenRouterContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenRouterToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenRouterToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: OpenRouterFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenRouterFunction {
    name: String,
    arguments: String,
}

#[derive(Debug, Serialize)]
struct OpenRouterTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenRouterToolFunction,
}

#[derive(Debug, Serialize)]
struct OpenRouterToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct OpenRouterUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponse {
    choices: Vec<OpenRouterChoice>,
    #[serde(default)]
    usage: Option<OpenRouterUsage>,
    /// The actual model used (may differ from requested if fallback occurred)
    #[serde(default)]
    model: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterResponseMessage,
}

#[derive(Debug, Deserialize)]
struct OpenRouterResponseMessage {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenRouterToolCall>,
}

impl OpenRouterClient {
    pub fn new(api_key: String, model: String, max_tokens: usize) -> Self {
        Self {
            api_key,
            model,
            max_tokens,
            client: reqwest::Client::new(),
            site_url: Some("https://github.com/siddharth-ghatti/safe-coder".to_string()),
            site_name: Some("Safe-Coder".to_string()),
        }
    }

    /// Create with custom site info for OpenRouter rankings
    pub fn with_site_info(
        api_key: String,
        model: String,
        max_tokens: usize,
        site_url: Option<String>,
        site_name: Option<String>,
    ) -> Self {
        Self {
            api_key,
            model,
            max_tokens,
            client: reqwest::Client::new(),
            site_url,
            site_name,
        }
    }

    /// Convert and validate messages using shared OpenAI-compatible logic
    fn prepare_messages(&self, messages: &[Message]) -> Vec<OpenRouterMessage> {
        // Use shared conversion logic
        let compat_messages = openai_compat::convert_messages(messages);
        // Validate tool call/result pairs
        let validated = openai_compat::validate_tool_pairs(compat_messages);
        // Convert to OpenRouter-specific format
        validated.into_iter().map(Self::from_compat_message).collect()
    }

    /// Convert from shared format to OpenRouter-specific format
    fn from_compat_message(msg: OpenAiCompatMessage) -> OpenRouterMessage {
        // Handle multimodal content (images) vs simple text
        let content = if let Some(parts) = msg.content_parts {
            // Multimodal: convert to content parts array
            let openrouter_parts: Vec<OpenRouterContentPart> = parts
                .into_iter()
                .map(|part| match part {
                    openai_compat::OpenAiContentPart::Text { text } => {
                        OpenRouterContentPart::Text { text }
                    }
                    openai_compat::OpenAiContentPart::ImageUrl { url } => {
                        OpenRouterContentPart::ImageUrl {
                            image_url: OpenRouterImageUrlData { url },
                        }
                    }
                })
                .collect();
            Some(OpenRouterContent::Parts(openrouter_parts))
        } else {
            // Simple text content
            msg.content.map(OpenRouterContent::Text)
        };

        OpenRouterMessage {
            role: msg.role,
            content,
            tool_calls: msg.tool_calls.map(|calls| {
                calls.into_iter().map(|tc| OpenRouterToolCall {
                    id: tc.id,
                    call_type: tc.call_type,
                    function: OpenRouterFunction {
                        name: tc.function_name,
                        arguments: tc.function_arguments,
                    },
                }).collect()
            }),
            tool_call_id: msg.tool_call_id,
            name: None,
        }
    }
}

#[async_trait]
impl LlmClient for OpenRouterClient {
    async fn send_message_with_system(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system_prompt: Option<&str>,
    ) -> Result<LlmResponse> {
        // Build messages, prepending system prompt if provided
        let mut openrouter_messages = Vec::new();

        if let Some(system) = system_prompt {
            openrouter_messages.push(OpenRouterMessage {
                role: "system".to_string(),
                content: Some(OpenRouterContent::Text(system.to_string())),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            });
        }

        openrouter_messages.extend(self.prepare_messages(messages));

        let openrouter_tools: Vec<OpenRouterTool> = tools
            .iter()
            .map(|tool| OpenRouterTool {
                tool_type: "function".to_string(),
                function: OpenRouterToolFunction {
                    name: tool.name.clone(),
                    description: tool.description.clone(),
                    parameters: tool.input_schema.clone(),
                },
            })
            .collect();

        let request = OpenRouterRequest {
            model: self.model.clone(),
            messages: openrouter_messages,
            tools: openrouter_tools,
            max_tokens: Some(self.max_tokens),
            route: Some("fallback".to_string()), // Allow fallback to similar models
        };

        let url = format!("{}/chat/completions", OPENROUTER_API_URL);

        // Build request with OpenRouter-specific headers
        let mut req = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json");

        // Add optional OpenRouter headers for rankings/attribution
        if let Some(ref site_url) = self.site_url {
            req = req.header("HTTP-Referer", site_url);
        }
        if let Some(ref site_name) = self.site_name {
            req = req.header("X-Title", site_name);
        }

        let response = req
            .json(&request)
            .send()
            .await
            .context("Failed to send request to OpenRouter")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            anyhow::bail!("OpenRouter API error ({}): {}", status, error_text);
        }

        let openrouter_response: OpenRouterResponse = response
            .json()
            .await
            .context("Failed to parse OpenRouter response")?;

        // Log if a different model was used (fallback)
        if let Some(ref actual_model) = openrouter_response.model {
            if actual_model != &self.model {
                tracing::info!(
                    "OpenRouter used fallback model: {} (requested: {})",
                    actual_model,
                    self.model
                );
            }
        }

        let choice = openrouter_response
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("No response from OpenRouter"))?;

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

        // Extract token usage
        let usage = openrouter_response
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

/// Popular OpenRouter models for quick reference
pub mod models {
    // Anthropic
    pub const CLAUDE_3_5_SONNET: &str = "anthropic/claude-3.5-sonnet";
    pub const CLAUDE_3_OPUS: &str = "anthropic/claude-3-opus";
    pub const CLAUDE_3_HAIKU: &str = "anthropic/claude-3-haiku";

    // OpenAI
    pub const GPT_4O: &str = "openai/gpt-4o";
    pub const GPT_4O_MINI: &str = "openai/gpt-4o-mini";
    pub const GPT_4_TURBO: &str = "openai/gpt-4-turbo";

    // Google
    pub const GEMINI_PRO_1_5: &str = "google/gemini-pro-1.5";
    pub const GEMINI_FLASH_1_5: &str = "google/gemini-flash-1.5";

    // Meta
    pub const LLAMA_3_1_405B: &str = "meta-llama/llama-3.1-405b-instruct";
    pub const LLAMA_3_1_70B: &str = "meta-llama/llama-3.1-70b-instruct";
    pub const LLAMA_3_1_8B: &str = "meta-llama/llama-3.1-8b-instruct";

    // Mistral
    pub const MIXTRAL_8X22B: &str = "mistralai/mixtral-8x22b-instruct";
    pub const MISTRAL_LARGE: &str = "mistralai/mistral-large";

    // DeepSeek
    pub const DEEPSEEK_CODER: &str = "deepseek/deepseek-coder";
    pub const DEEPSEEK_CHAT: &str = "deepseek/deepseek-chat";

    // Qwen
    pub const QWEN_2_72B: &str = "qwen/qwen-2-72b-instruct";
}
