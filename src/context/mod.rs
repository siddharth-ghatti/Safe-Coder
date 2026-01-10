//! Context Compaction
//!
//! Manages conversation context to prevent token overflow.
//! Based on Codex CLI's approach with token-based preservation.
//!
//! When context grows too large, it compacts by:
//! 1. Preserving recent tokens (not just message count)
//! 2. Summarizing older messages
//! 3. Pruning large tool results
//! 4. Providing warnings about potential accuracy degradation

use crate::llm::{ContentBlock, Message, Role};

/// Configuration for context compaction
/// Based on Codex CLI's approach with configurable thresholds
#[derive(Debug, Clone)]
pub struct ContextConfig {
    /// Maximum estimated tokens before triggering compaction
    pub max_tokens: usize,
    /// Trigger compaction at this percentage of max_tokens (0-100)
    pub compact_threshold_pct: usize,
    /// Number of recent tokens to preserve during compaction
    /// Similar to Codex's ~20k token preservation
    pub preserve_recent_tokens: usize,
    /// Minimum number of recent messages to always preserve (safety floor)
    pub min_preserve_messages: usize,
    /// Maximum size of tool results before truncation (chars)
    pub max_tool_result_chars: usize,
    /// Show warning after this many compactions in a session
    pub compaction_warning_threshold: usize,
    /// Average characters per token (rough estimate)
    pub chars_per_token: usize,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_tokens: 128_000,               // Claude's context window
            compact_threshold_pct: 60,         // Compact at 60% to leave room for responses
            preserve_recent_tokens: 20_000,    // Keep ~20k tokens like Codex
            min_preserve_messages: 5,          // Always keep at least 5 messages
            max_tool_result_chars: 2000,       // Truncate large tool results
            compaction_warning_threshold: 3,   // Warn after 3 compactions
            chars_per_token: 4,                // Rough estimate
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

/// Result of a compaction operation
#[derive(Debug, Clone)]
pub struct CompactionResult {
    /// Number of messages removed
    pub messages_removed: usize,
    /// Estimated tokens before compaction
    pub tokens_before: usize,
    /// Estimated tokens after compaction
    pub tokens_after: usize,
    /// Human-readable summary of what was compacted
    pub summary: String,
}

impl CompactionResult {
    /// Check if any compaction occurred
    pub fn did_compact(&self) -> bool {
        self.messages_removed > 0
    }

    /// Get the number of tokens saved
    pub fn tokens_saved(&self) -> usize {
        self.tokens_before.saturating_sub(self.tokens_after)
    }
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
    /// Uses token-based preservation (like Codex) instead of message count
    /// Returns the compacted messages, summary, and compaction metadata
    pub fn compact(&self, messages: Vec<Message>) -> (Vec<Message>, CompactionResult) {
        // Calculate tokens per message
        let message_tokens: Vec<usize> = messages
            .iter()
            .map(|msg| self.estimate_message_tokens(msg))
            .collect();

        let total_tokens: usize = message_tokens.iter().sum();

        // Find split point based on token preservation
        let split_point = self.find_token_based_split(&messages, &message_tokens);

        // Ensure we preserve minimum messages
        let adjusted_split = if messages.len() - split_point < self.config.min_preserve_messages {
            messages.len().saturating_sub(self.config.min_preserve_messages)
        } else {
            split_point
        };

        // If nothing to compact, return as-is
        if adjusted_split == 0 {
            return (
                messages,
                CompactionResult {
                    messages_removed: 0,
                    tokens_before: total_tokens,
                    tokens_after: total_tokens,
                    summary: String::new(),
                },
            );
        }

        // Adjust for safe split (don't break tool call/result pairs)
        let safe_split = self.find_safe_split_point(&messages, adjusted_split);

        let (old_messages, recent_messages) = messages.split_at(safe_split);

        let tokens_before = total_tokens;
        let tokens_removed: usize = message_tokens[..safe_split].iter().sum();

        let mut compacted = Vec::new();

        // Generate summary of old messages
        let old_summary = self.summarize_messages(old_messages);
        if !old_summary.is_empty() {
            // Add summary as a system-style user message
            compacted.push(Message {
                role: Role::User,
                content: vec![ContentBlock::Text {
                    text: format!(
                        "[Context Summary - {} earlier messages compacted (~{} tokens)]\n\n{}",
                        old_messages.len(),
                        tokens_removed,
                        old_summary
                    ),
                }],
            });
        }

        // Add recent messages, but prune large tool results
        for msg in recent_messages {
            compacted.push(self.prune_message(msg.clone()));
        }

        let tokens_after = self.analyze(&compacted).estimated_tokens;

        (
            compacted,
            CompactionResult {
                messages_removed: old_messages.len(),
                tokens_before,
                tokens_after,
                summary: format!(
                    "Compacted {} messages (~{} tokens)",
                    old_messages.len(),
                    tokens_removed
                ),
            },
        )
    }

    /// Estimate tokens for a single message
    fn estimate_message_tokens(&self, msg: &Message) -> usize {
        let mut chars = 0;
        for block in &msg.content {
            match block {
                ContentBlock::Text { text } => chars += text.len(),
                ContentBlock::ToolUse { name, input, .. } => {
                    chars += name.len();
                    chars += input.to_string().len();
                }
                ContentBlock::ToolResult { content, .. } => {
                    chars += content.len();
                }
            }
        }
        chars / self.config.chars_per_token
    }

    /// Find split point based on token preservation
    /// Preserves the last N tokens worth of messages
    fn find_token_based_split(&self, messages: &[Message], message_tokens: &[usize]) -> usize {
        let mut preserved_tokens = 0;
        let mut split_idx = messages.len();

        // Work backwards from the end, accumulating tokens
        for (i, tokens) in message_tokens.iter().enumerate().rev() {
            if preserved_tokens + tokens > self.config.preserve_recent_tokens {
                split_idx = i + 1;
                break;
            }
            preserved_tokens += tokens;
            split_idx = i;
        }

        split_idx
    }

    /// Summarize a set of messages into a compact form
    fn summarize_messages(&self, messages: &[Message]) -> String {
        let mut summary = String::new();

        // Extract key information from messages
        let mut user_requests = Vec::new();
        let mut assistant_actions = Vec::new();
        let mut files_modified = Vec::new();
        let mut files_read = Vec::new();
        let mut tools_used = Vec::new();
        let mut key_decisions = Vec::new();

        for msg in messages {
            for block in &msg.content {
                match block {
                    ContentBlock::Text { text } => {
                        // Capture user requests (from user messages)
                        if matches!(msg.role, Role::User) {
                            // Get first meaningful sentence
                            let first_part: String = text.chars().take(300).collect();
                            if let Some(sentence) = first_part.split(&['.', '?', '!'][..]).next() {
                                let trimmed = sentence.trim();
                                if trimmed.len() > 15 && !trimmed.starts_with('[') {
                                    user_requests.push(trimmed.to_string());
                                }
                            }
                        } else {
                            // Capture key assistant statements
                            for line in text.lines().take(3) {
                                let trimmed = line.trim();
                                if trimmed.len() > 20
                                    && trimmed.len() < 200
                                    && !trimmed.starts_with('[')
                                    && !trimmed.starts_with("```")
                                {
                                    assistant_actions.push(trimmed.to_string());
                                    break;
                                }
                            }
                        }

                        // Look for file paths mentioned
                        for word in text.split_whitespace() {
                            if word.contains('/') && word.contains('.') {
                                let clean = word.trim_matches(|c: char| {
                                    !c.is_alphanumeric()
                                        && c != '/'
                                        && c != '.'
                                        && c != '_'
                                        && c != '-'
                                });
                                if clean.len() > 3 && !clean.starts_with("http") {
                                    files_read.push(clean.to_string());
                                }
                            }
                        }
                    }
                    ContentBlock::ToolUse { name, input, .. } => {
                        if !tools_used.contains(name) {
                            tools_used.push(name.clone());
                        }
                        // Track file modifications
                        if name == "write" || name == "edit" || name == "Write" || name == "Edit" {
                            if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                                if !files_modified.contains(&path.to_string()) {
                                    files_modified.push(path.to_string());
                                }
                            }
                        }
                    }
                    ContentBlock::ToolResult { content, .. } => {
                        // Capture important results/errors
                        if content.contains("error")
                            || content.contains("Error")
                            || content.contains("failed")
                        {
                            let first_line = content.lines().next().unwrap_or("");
                            if first_line.len() > 10 && first_line.len() < 150 {
                                key_decisions.push(format!("Encountered: {}", first_line));
                            }
                        }
                    }
                }
            }
        }

        // Build comprehensive summary
        summary.push_str("=== Conversation Summary ===\n\n");

        if !user_requests.is_empty() {
            summary.push_str("User requested:\n");
            for (i, req) in user_requests.iter().take(8).enumerate() {
                summary.push_str(&format!("  {}. {}\n", i + 1, req));
            }
            if user_requests.len() > 8 {
                summary.push_str(&format!(
                    "  ... and {} more requests\n",
                    user_requests.len() - 8
                ));
            }
            summary.push('\n');
        }

        if !assistant_actions.is_empty() {
            summary.push_str("Actions taken:\n");
            for action in assistant_actions.iter().take(6) {
                summary.push_str(&format!("  - {}\n", action));
            }
            summary.push('\n');
        }

        if !files_modified.is_empty() {
            summary.push_str(&format!("Files modified: {}\n", files_modified.join(", ")));
        }

        // Dedupe files read
        files_read.sort();
        files_read.dedup();
        if !files_read.is_empty() {
            let display_files: Vec<_> = files_read.iter().take(15).cloned().collect();
            summary.push_str(&format!("Files referenced: {}\n", display_files.join(", ")));
            if files_read.len() > 15 {
                summary.push_str(&format!("  ... and {} more files\n", files_read.len() - 15));
            }
        }

        if !tools_used.is_empty() {
            summary.push_str(&format!("Tools used: {}\n", tools_used.join(", ")));
        }

        if !key_decisions.is_empty() {
            summary.push_str("\nKey events:\n");
            for decision in key_decisions.iter().take(5) {
                summary.push_str(&format!("  - {}\n", decision));
            }
        }

        summary.push_str("\n=== End Summary ===\n");
        summary
    }

    /// Find a safe split point that doesn't break tool call/result pairs
    /// OpenAI API requires that assistant messages with tool_calls are immediately
    /// followed by tool messages with matching tool_call_ids
    fn find_safe_split_point(&self, messages: &[Message], initial_split: usize) -> usize {
        use std::collections::HashSet;

        // Start from initial_split and look for a safe boundary
        let mut split_point = initial_split;

        // First, collect all tool_call_ids from messages before the split point
        let mut pending_tool_calls: HashSet<String> = HashSet::new();

        for (i, msg) in messages.iter().enumerate() {
            if i >= split_point {
                break;
            }

            for block in &msg.content {
                match block {
                    ContentBlock::ToolUse { id, .. } => {
                        pending_tool_calls.insert(id.clone());
                    }
                    ContentBlock::ToolResult { tool_use_id, .. } => {
                        pending_tool_calls.remove(tool_use_id);
                    }
                    _ => {}
                }
            }
        }

        // If there are pending tool calls before the split, we need to move the split
        // point earlier to include these tool calls with their results
        if !pending_tool_calls.is_empty() {
            // Move split point back to include tool results
            // Look for messages after split_point that have matching tool results
            let mut found_all_results = false;
            for i in split_point..messages.len() {
                for block in &messages[i].content {
                    if let ContentBlock::ToolResult { tool_use_id, .. } = block {
                        pending_tool_calls.remove(tool_use_id);
                    }
                }
                if pending_tool_calls.is_empty() {
                    // Found all results, split after this message
                    split_point = i + 1;
                    found_all_results = true;
                    break;
                }
            }

            // If we couldn't find all results, be conservative and keep more messages
            if !found_all_results {
                // Move split to include all tool calls that have results
                split_point = initial_split.saturating_sub(2);
            }
        }

        // Also check if we're splitting in the middle of a tool call sequence
        // by looking at messages right at the split boundary
        if split_point > 0 && split_point < messages.len() {
            // Check if the message right before split has tool_calls
            let prev_msg = &messages[split_point - 1];
            let has_pending_calls = prev_msg.content.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. }));

            if has_pending_calls {
                // Need to include at least the next message (should have tool results)
                // But verify it actually has the results
                if split_point < messages.len() {
                    let next_msg = &messages[split_point];
                    let has_results = next_msg.content.iter().any(|b| matches!(b, ContentBlock::ToolResult { .. }));
                    if has_results {
                        split_point += 1;
                    }
                }
            }
        }

        // Ensure we don't return a split point larger than the array
        split_point.min(messages.len())
    }

    /// Prune a single message to reduce size
    fn prune_message(&self, mut msg: Message) -> Message {
        let max_result_len = self.config.max_tool_result_chars;
        let pruned_content: Vec<ContentBlock> = msg
            .content
            .into_iter()
            .map(|block| match block {
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                } => {
                    // Truncate large tool results
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
    fn test_compact_preserves_recent_tokens() {
        // Configure to preserve only 100 tokens
        let manager = ContextManager::with_config(ContextConfig {
            preserve_recent_tokens: 100,
            min_preserve_messages: 2,
            ..Default::default()
        });

        // Create messages that exceed 100 tokens total
        // Each message is ~20 tokens (80 chars / 4 chars per token)
        let messages: Vec<Message> = (0..10)
            .map(|i| {
                make_text_message(
                    Role::User,
                    &format!(
                        "This is a longer message number {} with more content to analyze and fill up tokens.",
                        i
                    ),
                )
            })
            .collect();

        let (compacted, result) = manager.compact(messages);

        // Should have compacted some messages
        assert!(result.did_compact(), "Should have compacted messages");
        assert!(compacted.len() < 10, "Should have fewer messages after compaction");
        assert!(!result.summary.is_empty(), "Summary should not be empty after compaction");
    }

    #[test]
    fn test_no_compact_when_small() {
        let manager = ContextManager::with_config(ContextConfig {
            preserve_recent_tokens: 10_000, // Large enough to preserve everything
            min_preserve_messages: 5,
            ..Default::default()
        });

        let messages = vec![
            make_text_message(Role::User, "Hello"),
            make_text_message(Role::Assistant, "Hi there!"),
        ];

        let (compacted, result) = manager.compact(messages.clone());

        assert_eq!(compacted.len(), messages.len());
        assert!(!result.did_compact());
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
