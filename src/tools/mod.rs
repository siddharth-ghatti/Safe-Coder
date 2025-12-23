use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::config::ToolConfig;

pub mod read;
pub mod write;
pub mod edit;
pub mod bash;

pub use read::ReadTool;
pub use write::WriteTool;
pub use edit::EditTool;
pub use bash::BashTool;

/// Context passed to tool execution containing working directory and configuration
#[derive(Debug, Clone)]
pub struct ToolContext<'a> {
    pub working_dir: &'a Path,
    pub config: &'a ToolConfig,
}

impl<'a> ToolContext<'a> {
    pub fn new(working_dir: &'a Path, config: &'a ToolConfig) -> Self {
        Self { working_dir, config }
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
        registry.register(Box::new(ReadTool));
        registry.register(Box::new(WriteTool));
        registry.register(Box::new(EditTool));
        registry.register(Box::new(BashTool));
        registry
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.push(tool);
    }

    pub fn get_tool(&self, name: &str) -> Option<&dyn Tool> {
        self.tools.iter()
            .find(|t| t.name() == name)
            .map(|t| t.as_ref())
    }

    pub fn get_tools_schema(&self) -> Vec<serde_json::Value> {
        self.tools.iter()
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
