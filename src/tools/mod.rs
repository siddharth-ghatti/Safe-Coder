use anyhow::Result;
use async_trait::async_trait;
use std::fmt;
use std::path::Path;
use std::sync::Arc;

use crate::config::ToolConfig;

/// Agent execution mode - controls which tools are available
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentMode {
    /// Plan mode: Read-only exploration tools only
    /// Use this to explore, understand, and plan before making changes
    Plan,
    /// Build mode: Full tool access including file modifications and bash
    #[default]
    Build,
}

impl AgentMode {
    /// Get the list of tool names available in this mode
    pub fn enabled_tools(&self) -> &'static [&'static str] {
        match self {
            AgentMode::Plan => &[
                "read_file", // Read files
                "list_file", // List directories
                "glob",      // Find files by pattern
                "grep",      // Search file contents
                "webfetch",  // Fetch web content
                "todoread",  // Read task list
            ],
            AgentMode::Build => &[
                "read_file",
                "write_file",
                "edit_file",
                "list_file",
                "glob",
                "grep",
                "bash",
                "webfetch",
                "todowrite",
                "todoread",
            ],
        }
    }

    /// Check if a specific tool is enabled in this mode
    pub fn is_tool_enabled(&self, tool_name: &str) -> bool {
        self.enabled_tools().contains(&tool_name)
    }

    /// Get a description of this mode for display
    pub fn description(&self) -> &'static str {
        match self {
            AgentMode::Plan => {
                "Read-only exploration mode. Analyze the codebase before making changes."
            }
            AgentMode::Build => "Full execution mode. Can modify files and run commands.",
        }
    }

    /// Get short display name
    pub fn short_name(&self) -> &'static str {
        match self {
            AgentMode::Plan => "PLAN",
            AgentMode::Build => "BUILD",
        }
    }

    /// Cycle to next mode
    pub fn next(self) -> Self {
        match self {
            AgentMode::Plan => AgentMode::Build,
            AgentMode::Build => AgentMode::Plan,
        }
    }
}

impl fmt::Display for AgentMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.short_name())
    }
}

pub mod bash;
pub mod edit;
pub mod glob;
pub mod grep;
pub mod list;
pub mod read;
pub mod todo;
pub mod webfetch;
pub mod write;

pub use bash::BashTool;
pub use edit::EditTool;
pub use glob::GlobTool;
pub use grep::GrepTool;
pub use list::ListTool;
pub use read::ReadTool;
pub use todo::{TodoReadTool, TodoWriteTool};
pub use webfetch::WebFetchTool;
pub use write::WriteTool;

/// Callback type for streaming output updates
pub type OutputCallback = Arc<dyn Fn(String) + Send + Sync>;

/// Context passed to tool execution containing working directory and configuration
#[derive(Clone)]
pub struct ToolContext<'a> {
    pub working_dir: &'a Path,
    pub config: &'a ToolConfig,
    /// Optional callback for streaming output (used by bash tool)
    pub output_callback: Option<OutputCallback>,
}

impl<'a> ToolContext<'a> {
    pub fn new(working_dir: &'a Path, config: &'a ToolConfig) -> Self {
        Self {
            working_dir,
            config,
            output_callback: None,
        }
    }

    pub fn with_output_callback(
        working_dir: &'a Path,
        config: &'a ToolConfig,
        callback: OutputCallback,
    ) -> Self {
        Self {
            working_dir,
            config,
            output_callback: Some(callback),
        }
    }
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> serde_json::Value;
    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext<'_>) -> Result<String>;
}

pub struct ToolRegistry {
    tools: Vec<Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self { tools: vec![] };
        // File operations
        registry.register(Box::new(ReadTool));
        registry.register(Box::new(WriteTool));
        registry.register(Box::new(EditTool));
        registry.register(Box::new(ListTool));
        // Search tools
        registry.register(Box::new(GlobTool));
        registry.register(Box::new(GrepTool));
        // Shell execution
        registry.register(Box::new(BashTool));
        // Web access
        registry.register(Box::new(WebFetchTool));
        // Task tracking
        registry.register(Box::new(TodoWriteTool));
        registry.register(Box::new(TodoReadTool));
        registry
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    pub fn get_tool(&self, name: &str) -> Option<&dyn Tool> {
        self.tools
            .iter()
            .find(|t| t.name() == name)
            .map(|t| t.as_ref())
    }

    pub fn get_tools_schema(&self) -> Vec<serde_json::Value> {
        self.tools
            .iter()
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "input_schema": tool.parameters_schema()
                })
            })
            .collect()
    }

    /// Get tool schemas filtered by agent mode
    /// Only returns tools that are enabled for the given mode
    pub fn get_tools_schema_for_mode(&self, mode: AgentMode) -> Vec<serde_json::Value> {
        self.tools
            .iter()
            .filter(|tool| mode.is_tool_enabled(tool.name()))
            .map(|tool| {
                serde_json::json!({
                    "name": tool.name(),
                    "description": tool.description(),
                    "input_schema": tool.parameters_schema()
                })
            })
            .collect()
    }

    /// Check if a tool can be executed in the given mode
    pub fn can_execute_in_mode(&self, tool_name: &str, mode: AgentMode) -> bool {
        mode.is_tool_enabled(tool_name)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
