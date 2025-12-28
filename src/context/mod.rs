//! Context Compaction
//!
//! Manages conversation context to prevent token overflow.
//! When context grows too large, it compacts by:
//! 1. Summarizing older messages
//! 2. Pruning tool results (keeping recent ones)
//! 3. Preserving system context and recent messages

use crate::llm::{ContentBlock, Message, Role};

/// Configuration for context compaction
#[derive(Debug, Clone)]
pub struct ContextConfig {
    /// Maximum estimated tokens before triggering compaction
    pub max_tokens: usize,
    /// Trigger compaction at this percentage of max_tokens
    pub compact_threshold_pct: usize,
    /// Number of recent messages to preserve
    pub preserve_recent_messages: usize,
    /// Number of tool results to keep
    pub preserve_tool_results: usize,
    /// Average characters per token (rough estimate)
    pub chars_per_token: usize,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_tokens: 128_000,          // Claude's context window
            compact_threshold_pct: 75,    // Compact at 75%
            preserve_recent_messages: 10, // Keep last 10 messages
            preserve_tool_results: 5,     // Keep last 5 tool results
            chars_per_token: 4,           // Rough estimate
        }
    }
}

/// Result of context analysis
#[derive(Debug, Clone)]
pub struct ContextStats {
    /// Estimated total tokens
    pub estimated_tokens: usize,
    /// Number of messages
    pub message_count: usize,
    /// Number of tool calls
    pub tool_call_count: usize,
    /// Number of tool results
    pub tool_result_count: usize,
    /// Whether compaction is recommended
    pub needs_compaction: bool,
    /// Percentage of max context used
    pub context_usage_pct: usize,
}

/// Manages context compaction for a conversation
#[derive(Debug)]
pub struct ContextManager {
    config: ContextConfig,
}

impl ContextManager {
    /// Create a new context manager with default config
    pub fn new() -> Self {
        Self {
            config: ContextConfig::default(),
        }
    }

    /// Create a new context manager with custom config
    pub fn with_config(config: ContextConfig) -> Self {
        Self { config }
    }

    /// Set max tokens (useful when switching models)
    pub fn set_max_tokens(&mut self, max_tokens: usize) {
        self.config.max_tokens = max_tokens;
    }

    /// Analyze current context and return stats
    pub fn analyze(&self, messages: &[Message]) -> ContextStats {
        let mut total_chars = 0;
        let mut tool_call_count = 0;
        let mut tool_result_count = 0;

        for msg in messages {
            for block in &msg.content {
                match block {
                    ContentBlock::Text { text } => {
                        total_chars += text.len();
                    }
                    ContentBlock::ToolUse { name, input, .. } => {
                        tool_call_count += 1;
                        total_chars += name.len();
                        total_chars += input.to_string().len();
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        tool_result_count += 1;
                        total_chars += content.len();
                    }
                }
            }
        }

        let estimated_tokens = total_chars / self.config.chars_per_token;
        let context_usage_pct = (estimated_tokens * 100) / self.config.max_tokens;
        let needs_compaction = context_usage_pct >= self.config.compact_threshold_pct;

        ContextStats {
            estimated_tokens,
            message_count: messages.len(),
            tool_call_count,
            tool_result_count,
            needs_compaction,
            context_usage_pct,
        }
    }

    /// Check if compaction is needed
    pub fn needs_compaction(&self, messages: &[Message]) -> bool {
        self.analyze(messages).needs_compaction
    }

    /// Compact the context by pruning and summarizing
    /// Returns the compacted messages and a summary of what was removed
    pub fn compact(&self, messages: Vec<Message>) -> (Vec<Message>, String) {
        if messages.len() <= self.config.preserve_recent_messages {
            return (messages, String::new());
        }

        let mut compacted = Vec::new();
        let mut summary_parts = Vec::new();

        // Split messages: old (to summarize) and recent (to preserve)
        let split_point = messages
            .len()
            .saturating_sub(self.config.preserve_recent_messages);
        let (old_messages, recent_messages) = messages.split_at(split_point);

        // Generate summary of old messages
        let old_summary = self.summarize_messages(old_messages);
        if !old_summary.is_empty() {
            summary_parts.push(format!("Compacted {} messages", old_messages.len()));

            // Add summary as a system-style user message
            compacted.push(Message {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: format!(
                        "[Context Summary - {} earlier messages compacted]\n\n{}",
                        old_messages.len(),
                        old_summary
                    ),
                }],
            });
        }

        // Add recent messages, but prune large tool results
        for msg in recent_messages {
            compacted.push(self.prune_message(msg.clone()));
        }

        let summary = if summary_parts.is_empty() {
            String::new()
        } else {
            summary_parts.join("; ")
        };

        (compacted, summary)
    }

    /// Summarize a set of messages into a compact form
    fn summarize_messages(&self, messages: &[Message]) -> String {
        let mut summary = String::new();

        // Extract key information from messages
        let mut topics = Vec::new();
        let mut files_mentioned = Vec::new();
        let mut tools_used = Vec::new();

        for msg in messages {
            for block in &msg.content {
                match block {
                    ContentBlock::Text { text } => {
                        // Extract first sentence as topic indicator
                        if let Some(first_sentence) = text.split('.').next() {
                            let trimmed = first_sentence.trim();
                            if trimmed.len() > 10 && trimmed.len() < 200 {
                                topics.push(trimmed.to_string());
                            }
                        }

                        // Look for file paths mentioned
                        for word in text.split_whitespace() {
                            if word.contains('/') && word.contains('.') {
                                let clean = word.trim_matches(|c: char| {
                                    !c.is_alphanumeric() && c != '/' && c != '.' && c != '_'
                                });
                                if clean.len() > 3 {
                                    files_mentioned.push(clean.to_string());
                                }
                            }
                        }
                    }
                    ContentBlock::ToolUse { name, .. } => {
                        if !tools_used.contains(name) {
                            tools_used.push(name.clone());
                        }
                    }
                    _ => {}
                }
            }
        }

        // Build summary
        if !topics.is_empty() {
            summary.push_str("Discussion covered:\n");
            for (i, topic) in topics.iter().take(5).enumerate() {
                summary.push_str(&format!("{}. {}\n", i + 1, topic));
            }
            if topics.len() > 5 {
                summary.push_str(&format!("... and {} more topics\n", topics.len() - 5));
            }
            summary.push('\n');
        }

        if !tools_used.is_empty() {
            summary.push_str(&format!("Tools used: {}\n", tools_used.join(", ")));
        }

        // Dedupe files
        files_mentioned.sort();
        files_mentioned.dedup();
        if !files_mentioned.is_empty() {
            summary.push_str(&format!(
                "Files referenced: {}\n",
                files_mentioned
                    .iter()
                    .take(10)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            if files_mentioned.len() > 10 {
                summary.push_str(&format!(
                    "... and {} more files\n",
                    files_mentioned.len() - 10
                ));
            }
        }

        summary
    }

    /// Prune a single message to reduce size
    fn prune_message(&self, mut msg: Message) -> Message {
        let pruned_content: Vec<ContentBlock> = msg
            .content
            .into_iter()
            .map(|block| match block {
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                } => {
                    // Truncate large tool results
                    let max_result_len = 2000;
                    if content.len() > max_result_len {
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content: format!(
                                "{}...\n\n[Truncated: {} chars total]",
                                &content[..max_result_len],
                                content.len()
                            ),
                        }
                    } else {
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                        }
                    }
                }
                other => other,
            })
            .collect();

        msg.content = pruned_content;
        msg
    }

    /// Get context usage as a formatted string
    pub fn usage_display(&self, messages: &[Message]) -> String {
        let stats = self.analyze(messages);
        format!(
            "Context: ~{}k/{:.0}k tokens ({}%) | {} msgs | {} tool calls",
            stats.estimated_tokens / 1000,
            self.config.max_tokens as f64 / 1000.0,
            stats.context_usage_pct,
            stats.message_count,
            stats.tool_call_count
        )
    }
}

impl Default for ContextManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_text_message(role: Role, text: &str) -> Message {
        Message {
            role,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        }
    }

    #[test]
    fn test_analyze_empty() {
        let manager = ContextManager::new();
        let stats = manager.analyze(&[]);

        assert_eq!(stats.message_count, 0);
        assert_eq!(stats.estimated_tokens, 0);
        assert!(!stats.needs_compaction);
    }

    #[test]
    fn test_analyze_basic() {
        let manager = ContextManager::new();
        let messages = vec![
            make_text_message(Role::User, "Hello, how are you?"),
            make_text_message(Role::Assistant, "I'm doing well, thank you!"),
        ];

        let stats = manager.analyze(&messages);

        assert_eq!(stats.message_count, 2);
        assert!(stats.estimated_tokens > 0);
        assert!(!stats.needs_compaction);
    }

    #[test]
    fn test_compact_preserves_recent() {
        let manager = ContextManager::with_config(ContextConfig {
            preserve_recent_messages: 3,
            ..Default::default()
        });

        // Use longer messages so they get included in summary
        let messages: Vec<Message> = (0..10)
            .map(|i| {
                make_text_message(
                    Role::User,
                    &format!(
                        "This is a longer message number {} with more content to analyze.",
                        i
                    ),
                )
            })
            .collect();

        let (compacted, summary) = manager.compact(messages);

        // Should have summary + 3 recent messages = 4
        assert!(compacted.len() <= 4);
        // Summary should exist because we compacted messages
        assert!(
            !summary.is_empty(),
            "Summary should not be empty after compaction"
        );
    }

    #[test]
    fn test_no_compact_when_small() {
        let manager = ContextManager::with_config(ContextConfig {
            preserve_recent_messages: 5,
            ..Default::default()
        });

        let messages = vec![
            make_text_message(Role::User, "Hello"),
            make_text_message(Role::Assistant, "Hi there!"),
        ];

        let (compacted, summary) = manager.compact(messages.clone());

        assert_eq!(compacted.len(), messages.len());
        assert!(summary.is_empty());
    }

    #[test]
    fn test_prune_large_tool_result() {
        let manager = ContextManager::new();
        let large_content = "x".repeat(5000);

        let msg = Message {
            role: Role::User,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "test".to_string(),
                content: large_content,
            }],
        };

        let pruned = manager.prune_message(msg);

        if let ContentBlock::ToolResult { content, .. } = &pruned.content[0] {
            assert!(content.len() < 5000);
            assert!(content.contains("[Truncated"));
        } else {
            panic!("Expected ToolResult");
        }
    }
}
