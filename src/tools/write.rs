use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;


use super::{Tool, ToolContext};

pub struct WriteTool;

#[derive(Debug, Deserialize)]
struct WriteParams {
    file_path: String,
    content: String,
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Writes content to a file. Creates the file if it doesn't exist, overwrites if it does."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path to the file to write (relative to project root)"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext<'_>) -> Result<String> {
        let params: WriteParams = serde_json::from_value(params)
            .context("Invalid parameters for write_file")?;

        let file_path = ctx.working_dir.join(&params.file_path);

        // Create parent directories if they don't exist
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&file_path, &params.content)
            .context("Failed to write file")?;

        Ok(format!("Successfully wrote to {}", params.file_path))
    }
}
