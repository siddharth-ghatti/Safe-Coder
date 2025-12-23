use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;

use super::{Tool, ToolContext};

pub struct ReadTool;

#[derive(Debug, Deserialize)]
struct ReadParams {
    file_path: String,
    #[serde(default)]
    offset: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Reads a file from the filesystem. Returns the contents with line numbers."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path to the file to read (relative to project root)"
                },
                "offset": {
                    "type": "number",
                    "description": "The line number to start reading from (optional)"
                },
                "limit": {
                    "type": "number",
                    "description": "The number of lines to read (optional)"
                }
            },
            "required": ["file_path"]
        })
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext<'_>) -> Result<String> {
        let params: ReadParams = serde_json::from_value(params)
            .context("Invalid parameters for read_file")?;

        let file_path = ctx.working_dir.join(&params.file_path);

        if !file_path.exists() {
            anyhow::bail!("File not found: {}", params.file_path);
        }

        let content = std::fs::read_to_string(&file_path)
            .context("Failed to read file")?;

        let lines: Vec<&str> = content.lines().collect();
        let offset = params.offset.unwrap_or(0);
        let limit = params.limit.unwrap_or(lines.len());

        let selected_lines = lines.iter()
            .skip(offset)
            .take(limit)
            .enumerate()
            .map(|(i, line)| format!("{:5}→{}", offset + i + 1, line))
            .collect::<Vec<_>>()
            .join("\n");

        Ok(selected_lines)
    }
}
