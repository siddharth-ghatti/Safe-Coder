//! Hook Types
//!
//! Core types for the hooks system including hook types, contexts, and results.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Types of lifecycle hooks available
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookType {
    /// Before a tool is executed
    PreToolUse,
    /// After a tool completes execution
    PostToolUse,
    /// Before sending a prompt to the LLM
    PrePrompt,
    /// After receiving a response from the LLM
    PostResponse,
    /// When a session starts
    SessionStart,
    /// When a session ends
    SessionEnd,
    /// Before file is written/edited
    PreFileWrite,
    /// After file is written/edited
    PostFileWrite,
    /// When context compaction is triggered
    OnCompaction,
}

impl HookType {
    /// Get all hook types
    pub fn all() -> &'static [HookType] {
        &[
            HookType::PreToolUse,
            HookType::PostToolUse,
            HookType::PrePrompt,
            HookType::PostResponse,
            HookType::SessionStart,
            HookType::SessionEnd,
            HookType::PreFileWrite,
            HookType::PostFileWrite,
            HookType::OnCompaction,
        ]
    }

    /// Get display name for this hook type
    pub fn display_name(&self) -> &'static str {
        match self {
            HookType::PreToolUse => "Pre-Tool Use",
            HookType::PostToolUse => "Post-Tool Use",
            HookType::PrePrompt => "Pre-Prompt",
            HookType::PostResponse => "Post-Response",
            HookType::SessionStart => "Session Start",
            HookType::SessionEnd => "Session End",
            HookType::PreFileWrite => "Pre-File Write",
            HookType::PostFileWrite => "Post-File Write",
            HookType::OnCompaction => "On Compaction",
        }
    }
}

/// Context passed to hooks during execution
#[derive(Debug, Clone)]
pub struct HookContext {
    /// The type of hook being executed
    pub hook_type: HookType,
    /// Tool name (for tool-related hooks)
    pub tool_name: Option<String>,
    /// Tool input (for PreToolUse)
    pub tool_input: Option<serde_json::Value>,
    /// Tool output (for PostToolUse)
    pub tool_output: Option<String>,
    /// File path (for file-related hooks)
    pub file_path: Option<String>,
    /// File content (for file-related hooks)
    pub file_content: Option<String>,
    /// The prompt being sent (for PrePrompt)
    pub prompt: Option<String>,
    /// The LLM response (for PostResponse)
    pub response: Option<String>,
    /// Current token usage
    pub token_usage: Option<TokenUsageInfo>,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl HookContext {
    /// Create a new empty context
    pub fn new(hook_type: HookType) -> Self {
        Self {
            hook_type,
            tool_name: None,
            tool_input: None,
            tool_output: None,
            file_path: None,
            file_content: None,
            prompt: None,
            response: None,
            token_usage: None,
            metadata: HashMap::new(),
        }
    }

    /// Create context for a tool use hook
    pub fn for_tool(
        hook_type: HookType,
        tool_name: &str,
        input: Option<serde_json::Value>,
    ) -> Self {
        Self {
            hook_type,
            tool_name: Some(tool_name.to_string()),
            tool_input: input,
            tool_output: None,
            file_path: None,
            file_content: None,
            prompt: None,
            response: None,
            token_usage: None,
            metadata: HashMap::new(),
        }
    }

    /// Create context for a file write hook
    pub fn for_file(hook_type: HookType, file_path: &str, content: Option<&str>) -> Self {
        Self {
            hook_type,
            tool_name: None,
            tool_input: None,
            tool_output: None,
            file_path: Some(file_path.to_string()),
            file_content: content.map(|s| s.to_string()),
            prompt: None,
            response: None,
            token_usage: None,
            metadata: HashMap::new(),
        }
    }

    /// Add tool output (for PostToolUse)
    pub fn with_tool_output(mut self, output: &str) -> Self {
        self.tool_output = Some(output.to_string());
        self
    }

    /// Add token usage information
    pub fn with_token_usage(mut self, usage: TokenUsageInfo) -> Self {
        self.token_usage = Some(usage);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

/// Token usage information for hooks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageInfo {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub total_tokens: usize,
    pub max_tokens: usize,
    pub usage_percent: f32,
}

impl TokenUsageInfo {
    pub fn new(input: usize, output: usize, max: usize) -> Self {
        let total = input + output;
        let usage_percent = if max > 0 {
            (total as f32 / max as f32) * 100.0
        } else {
            0.0
        };
        Self {
            input_tokens: input,
            output_tokens: output,
            total_tokens: total,
            max_tokens: max,
            usage_percent,
        }
    }
}

/// Result of a hook execution
#[derive(Debug, Clone)]
pub enum HookResult {
    /// Continue with execution as normal
    Continue,
    /// Continue but with a warning message
    ContinueWithWarning(String),
    /// Skip the action (e.g., don't execute the tool)
    Skip(String),
    /// Block the action and return an error
    Block(String),
    /// Modify the input/content and continue
    Modify {
        /// Modified content (interpretation depends on hook type)
        content: String,
        /// Optional message to display
        message: Option<String>,
    },
}

impl HookResult {
    /// Check if this result allows continuation
    pub fn should_continue(&self) -> bool {
        matches!(
            self,
            HookResult::Continue | HookResult::ContinueWithWarning(_) | HookResult::Modify { .. }
        )
    }

    /// Check if this result blocks the action
    pub fn is_blocked(&self) -> bool {
        matches!(self, HookResult::Block(_) | HookResult::Skip(_))
    }

    /// Get any message from the result
    pub fn message(&self) -> Option<&str> {
        match self {
            HookResult::Continue => None,
            HookResult::ContinueWithWarning(msg) => Some(msg),
            HookResult::Skip(msg) => Some(msg),
            HookResult::Block(msg) => Some(msg),
            HookResult::Modify { message, .. } => message.as_deref(),
        }
    }
}

/// Trait for implementing hooks
#[async_trait]
pub trait Hook: Send + Sync {
    /// Get the name of this hook
    fn name(&self) -> &str;

    /// Get the hook types this hook responds to
    fn hook_types(&self) -> &[HookType];

    /// Check if this hook is enabled
    fn is_enabled(&self) -> bool {
        true
    }

    /// Execute the hook
    async fn execute(&self, ctx: &HookContext) -> HookResult;

    /// Get a description of what this hook does
    fn description(&self) -> &str {
        "No description available"
    }
}
