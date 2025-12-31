//! Subagent Types
//!
//! Core types for the subagent system including kinds, scopes, results, and events.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Types of specialized subagents available
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SubagentKind {
    /// Analyzes code structure, patterns, and potential issues (read-only)
    CodeAnalyzer,
    /// Creates and runs tests
    Tester,
    /// Makes targeted code improvements and refactoring
    Refactorer,
    /// Generates and updates documentation
    Documenter,
    /// Custom subagent with user-defined role
    Custom,
}

impl SubagentKind {
    /// Get allowed tools for this subagent kind
    pub fn allowed_tools(&self) -> &'static [&'static str] {
        match self {
            SubagentKind::CodeAnalyzer => &["read_file", "list_file", "glob", "grep"],
            SubagentKind::Tester => &[
                "read_file",
                "list_file",
                "glob",
                "grep",
                "write_file",
                "bash",
            ],
            SubagentKind::Refactorer => &["read_file", "list_file", "glob", "grep", "edit_file"],
            SubagentKind::Documenter => &[
                "read_file",
                "list_file",
                "glob",
                "grep",
                "write_file",
                "edit_file",
            ],
            SubagentKind::Custom => &["read_file", "list_file", "glob", "grep"],
        }
    }

    /// Check if a tool is allowed for this subagent kind
    pub fn is_tool_allowed(&self, tool_name: &str) -> bool {
        self.allowed_tools().contains(&tool_name)
    }

    /// Get display name for this kind
    pub fn display_name(&self) -> &'static str {
        match self {
            SubagentKind::CodeAnalyzer => "Code Analyzer",
            SubagentKind::Tester => "Tester",
            SubagentKind::Refactorer => "Refactorer",
            SubagentKind::Documenter => "Documenter",
            SubagentKind::Custom => "Custom Agent",
        }
    }

    /// Get icon for this kind
    pub fn icon(&self) -> &'static str {
        match self {
            SubagentKind::CodeAnalyzer => "🔍",
            SubagentKind::Tester => "🧪",
            SubagentKind::Refactorer => "🔧",
            SubagentKind::Documenter => "📝",
            SubagentKind::Custom => "🤖",
        }
    }
}

impl std::fmt::Display for SubagentKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Scope and configuration for a subagent task
#[derive(Debug, Clone)]
pub struct SubagentScope {
    /// The specific task for the subagent to accomplish
    pub task: String,
    /// Optional custom role description (for Custom kind)
    pub role: Option<String>,
    /// Optional file patterns to focus on (e.g., ["src/**/*.rs"])
    pub file_patterns: Vec<String>,
    /// Maximum execution time (default: 5 minutes)
    pub timeout: Duration,
    /// Maximum iterations in the conversation loop (default: 15)
    pub max_iterations: usize,
}

impl SubagentScope {
    /// Create a new scope with just a task
    pub fn new(task: impl Into<String>) -> Self {
        Self {
            task: task.into(),
            role: None,
            file_patterns: Vec::new(),
            timeout: Duration::from_secs(300), // 5 minutes
            max_iterations: 15,
        }
    }

    /// Set custom role for Custom kind
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.role = Some(role.into());
        self
    }

    /// Set file patterns to focus on
    pub fn with_file_patterns(mut self, patterns: Vec<String>) -> Self {
        self.file_patterns = patterns;
        self
    }

    /// Set timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set max iterations
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = max;
        self
    }
}

/// Result from a subagent execution
#[derive(Debug, Clone)]
pub struct SubagentResult {
    /// Whether the subagent completed successfully
    pub success: bool,
    /// Summary of what was accomplished
    pub summary: String,
    /// Detailed output/findings
    pub output: String,
    /// Number of iterations used
    pub iterations: usize,
    /// Files that were read
    pub files_read: Vec<String>,
    /// Files that were modified
    pub files_modified: Vec<String>,
    /// Any errors encountered
    pub errors: Vec<String>,
}

impl SubagentResult {
    /// Create a successful result
    pub fn success(summary: impl Into<String>, output: impl Into<String>) -> Self {
        Self {
            success: true,
            summary: summary.into(),
            output: output.into(),
            iterations: 0,
            files_read: Vec::new(),
            files_modified: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Create a failed result
    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            success: false,
            summary: "Subagent failed".to_string(),
            output: String::new(),
            iterations: 0,
            files_read: Vec::new(),
            files_modified: Vec::new(),
            errors: vec![error.into()],
        }
    }

    /// Format result for display
    pub fn format(&self) -> String {
        let status = if self.success { "✓" } else { "✗" };
        let mut result = format!("{} {}\n", status, self.summary);

        if !self.output.is_empty() {
            result.push_str(&format!("\n{}\n", self.output));
        }

        if !self.files_modified.is_empty() {
            result.push_str(&format!(
                "\nModified files: {}\n",
                self.files_modified.join(", ")
            ));
        }

        if !self.errors.is_empty() {
            result.push_str(&format!("\nErrors: {}\n", self.errors.join(", ")));
        }

        result
    }
}

/// Events emitted by a subagent during execution
#[derive(Debug, Clone)]
pub enum SubagentEvent {
    /// Subagent started
    Started {
        id: String,
        kind: SubagentKind,
        task: String,
    },
    /// Subagent is thinking/processing
    Thinking { id: String, message: String },
    /// Subagent is using a tool
    ToolStart {
        id: String,
        tool_name: String,
        description: String,
    },
    /// Tool produced output
    ToolOutput {
        id: String,
        tool_name: String,
        output: String,
    },
    /// Tool completed
    ToolComplete {
        id: String,
        tool_name: String,
        success: bool,
    },
    /// Subagent produced text output
    TextChunk { id: String, text: String },
    /// Subagent completed an iteration
    IterationComplete {
        id: String,
        iteration: usize,
        max_iterations: usize,
    },
    /// Subagent completed
    Completed {
        id: String,
        success: bool,
        summary: String,
    },
    /// Subagent encountered an error
    Error { id: String, error: String },
}

impl SubagentEvent {
    /// Get the subagent ID from any event
    pub fn id(&self) -> &str {
        match self {
            SubagentEvent::Started { id, .. } => id,
            SubagentEvent::Thinking { id, .. } => id,
            SubagentEvent::ToolStart { id, .. } => id,
            SubagentEvent::ToolOutput { id, .. } => id,
            SubagentEvent::ToolComplete { id, .. } => id,
            SubagentEvent::TextChunk { id, .. } => id,
            SubagentEvent::IterationComplete { id, .. } => id,
            SubagentEvent::Completed { id, .. } => id,
            SubagentEvent::Error { id, .. } => id,
        }
    }
}
