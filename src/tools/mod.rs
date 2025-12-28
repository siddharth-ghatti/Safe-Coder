use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::config::ToolConfig;

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

    pub fn with_output_callback(working_dir: &'a Path, config: &'a ToolConfig, callback: OutputCallback) -> Self {
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
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
