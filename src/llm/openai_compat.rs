//! Shared utilities for OpenAI-compatible API providers
//!
//! This module provides common message conversion and validation logic
//! used by OpenAI, Copilot, OpenRouter, and Ollama clients.

use std::collections::HashSet;

use super::{ContentBlock, Message, Role};

/// A generic OpenAI-compatible message structure
#[derive(Debug, Clone)]
pub struct OpenAiCompatMessage {
    pub role: String,
    /// Simple text content (for non-multimodal messages)
    pub content: Option<String>,
    /// Multimodal content parts (for messages with images)
    pub content_parts: Option<Vec<OpenAiContentPart>>,
    pub tool_calls: Option<Vec<OpenAiCompatToolCall>>,
    pub tool_call_id: Option<String>,
}

/// Content part for multimodal OpenAI messages
#[derive(Debug, Clone)]
pub enum OpenAiContentPart {
    Text { text: String },
    ImageUrl { url: String },
}

/// A generic OpenAI-compatible tool call
#[derive(Debug, Clone)]
pub struct OpenAiCompatToolCall {
    pub id: String,
    pub call_type: String,
    pub function_name: String,
    pub function_arguments: String,
}

/// Convert internal Message format to OpenAI-compatible messages
///
/// This handles:
/// - Grouping multiple tool calls into a single assistant message
/// - Ensuring tool results come before text content (for proper ordering)
/// - Converting between Anthropic-style and OpenAI-style message formats
/// - Converting images to OpenAI's multimodal content format
pub fn convert_messages(messages: &[Message]) -> Vec<OpenAiCompatMessage> {
    let mut result = Vec::new();

    for msg in messages {
        let role = match msg.role {
            Role::User => "user",
            Role::Assistant => "assistant",
        };

        // Collect all tool calls, text, images from this message
        let mut tool_calls = Vec::new();
        let mut text_content = String::new();
        let mut tool_results = Vec::new();
        let mut images: Vec<(String, String)> = Vec::new(); // (data, media_type)

        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => {
                    if !text_content.is_empty() {
                        text_content.push('\n');
                    }
                    text_content.push_str(text);
                }
                ContentBlock::Image { data, media_type } => {
                    images.push((data.clone(), media_type.clone()));
                }
                ContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(OpenAiCompatToolCall {
                        id: id.clone(),
                        call_type: "function".to_string(),
                        function_name: name.clone(),
                        function_arguments: serde_json::to_string(input).unwrap_or_default(),
                    });
                }
                ContentBlock::ToolResult { tool_use_id, content } => {
                    tool_results.push((tool_use_id.clone(), content.clone()));
                }
            }
        }

        // Handle assistant messages with tool calls
        if !tool_calls.is_empty() {
            // OpenAI requires all tool_calls in one assistant message
            result.push(OpenAiCompatMessage {
                role: "assistant".to_string(),
                content: if text_content.is_empty() { None } else { Some(text_content.clone()) },
                content_parts: None,
                tool_calls: Some(tool_calls),
                tool_call_id: None,
            });
        } else {
            // IMPORTANT: Tool results must come BEFORE any text in the same message
            // OpenAI API requires tool responses immediately after assistant tool_calls
            // So we add tool results first, then any text content

            // Tool results must be separate messages with role "tool"
            for (tool_use_id, content) in tool_results {
                result.push(OpenAiCompatMessage {
                    role: "tool".to_string(),
                    content: Some(content),
                    content_parts: None,
                    tool_calls: None,
                    tool_call_id: Some(tool_use_id),
                });
            }

            // Then add any text/image content
            let has_images = !images.is_empty();
            let has_text = !text_content.is_empty();

            if has_images || has_text {
                if has_images {
                    // Use multimodal content_parts format
                    let mut parts = Vec::new();

                    // Add text first if present
                    if has_text {
                        parts.push(OpenAiContentPart::Text { text: text_content });
                    }

                    // Add images as data URLs
                    for (data, media_type) in images {
                        let data_url = format!("data:{};base64,{}", media_type, data);
                        parts.push(OpenAiContentPart::ImageUrl { url: data_url });
                    }

                    result.push(OpenAiCompatMessage {
                        role: role.to_string(),
                        content: None,
                        content_parts: Some(parts),
                        tool_calls: None,
                        tool_call_id: None,
                    });
                } else {
                    // Text-only message
                    result.push(OpenAiCompatMessage {
                        role: role.to_string(),
                        content: Some(text_content),
                        content_parts: None,
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
            }

            // Skip the tool_results loop below since we already processed them
            continue;
        }

        // Tool results for assistant messages with tool_calls (rare case)
        for (tool_use_id, content) in tool_results {
            result.push(OpenAiCompatMessage {
                role: "tool".to_string(),
                content: Some(content),
                content_parts: None,
                tool_calls: None,
                tool_call_id: Some(tool_use_id),
            });
        }
    }

    result
}

/// Validate that all tool_calls have matching tool results and fix any issues
///
/// OpenAI API requires: assistant message with tool_calls must be immediately
/// followed by tool messages with matching tool_call_ids.
///
/// This function:
/// 1. Identifies tool_calls without matching tool responses
/// 2. Removes or converts problematic messages to prevent API errors
pub fn validate_tool_pairs(messages: Vec<OpenAiCompatMessage>) -> Vec<OpenAiCompatMessage> {
    // First pass: collect all tool_call_ids and tool_call_id responses
    let mut expected_tool_ids: HashSet<String> = HashSet::new();
    let mut found_tool_ids: HashSet<String> = HashSet::new();

    for msg in &messages {
        if let Some(ref tool_calls) = msg.tool_calls {
            for tc in tool_calls {
                expected_tool_ids.insert(tc.id.clone());
            }
        }
        if msg.role == "tool" {
            if let Some(ref id) = msg.tool_call_id {
                found_tool_ids.insert(id.clone());
            }
        }
    }

    // Find missing tool responses
    let missing: HashSet<_> = expected_tool_ids.difference(&found_tool_ids).collect();

    if missing.is_empty() {
        return messages;
    }

    // Log the issue
    tracing::warn!(
        "Found {} tool_calls without responses, removing them: {:?}",
        missing.len(),
        missing
    );

    // Second pass: filter out messages with broken tool_calls
    let mut result = Vec::new();
    for msg in messages {
        if let Some(ref tool_calls) = msg.tool_calls {
            // Check if all tool_calls in this message have responses
            let has_all_responses = tool_calls.iter().all(|tc| found_tool_ids.contains(&tc.id));

            if !has_all_responses {
                // This message has tool_calls without responses
                // Convert it to a regular assistant message with just the text content
                if msg.content.is_some() {
                    result.push(OpenAiCompatMessage {
                        role: msg.role,
                        content: msg.content,
                        content_parts: None,
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
                // Skip this message entirely if it has no text content
                continue;
            }
        }
        result.push(msg);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_simple_text_messages() {
        let messages = vec![
            Message {
                role: Role::User,
                content: vec![ContentBlock::Text { text: "Hello".to_string() }],
            },
            Message {
                role: Role::Assistant,
                content: vec![ContentBlock::Text { text: "Hi there!".to_string() }],
            },
        ];

        let converted = convert_messages(&messages);
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].role, "user");
        assert_eq!(converted[0].content, Some("Hello".to_string()));
        assert_eq!(converted[1].role, "assistant");
        assert_eq!(converted[1].content, Some("Hi there!".to_string()));
    }

    #[test]
    fn test_tool_results_before_text() {
        // Simulate a user message with tool results and text (like build errors)
        let messages = vec![
            Message {
                role: Role::User,
                content: vec![
                    ContentBlock::ToolResult {
                        tool_use_id: "call_123".to_string(),
                        content: "File written successfully".to_string(),
                    },
                    ContentBlock::Text { text: "Build failed: error on line 5".to_string() },
                ],
            },
        ];

        let converted = convert_messages(&messages);

        // Tool result should come first
        assert_eq!(converted.len(), 2);
        assert_eq!(converted[0].role, "tool");
        assert_eq!(converted[0].tool_call_id, Some("call_123".to_string()));

        // Text should come second
        assert_eq!(converted[1].role, "user");
        assert!(converted[1].content.as_ref().unwrap().contains("Build failed"));
    }

    #[test]
    fn test_validate_removes_orphaned_tool_calls() {
        let messages = vec![
            OpenAiCompatMessage {
                role: "assistant".to_string(),
                content: Some("Let me help".to_string()),
                content_parts: None,
                tool_calls: Some(vec![OpenAiCompatToolCall {
                    id: "orphan_call".to_string(),
                    call_type: "function".to_string(),
                    function_name: "read_file".to_string(),
                    function_arguments: "{}".to_string(),
                }]),
                tool_call_id: None,
            },
            // No matching tool response!
        ];

        let validated = validate_tool_pairs(messages);

        // Should convert to text-only message
        assert_eq!(validated.len(), 1);
        assert_eq!(validated[0].role, "assistant");
        assert!(validated[0].tool_calls.is_none());
        assert_eq!(validated[0].content, Some("Let me help".to_string()));
    }

    #[test]
    fn test_validate_keeps_complete_pairs() {
        let messages = vec![
            OpenAiCompatMessage {
                role: "assistant".to_string(),
                content: None,
                content_parts: None,
                tool_calls: Some(vec![OpenAiCompatToolCall {
                    id: "call_456".to_string(),
                    call_type: "function".to_string(),
                    function_name: "read_file".to_string(),
                    function_arguments: "{}".to_string(),
                }]),
                tool_call_id: None,
            },
            OpenAiCompatMessage {
                role: "tool".to_string(),
                content: Some("file contents".to_string()),
                content_parts: None,
                tool_calls: None,
                tool_call_id: Some("call_456".to_string()),
            },
        ];

        let validated = validate_tool_pairs(messages.clone());

        // Should keep both messages unchanged
        assert_eq!(validated.len(), 2);
        assert!(validated[0].tool_calls.is_some());
    }
}
