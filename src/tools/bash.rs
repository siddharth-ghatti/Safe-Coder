use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;
use tokio::process::Command;

use super::Tool;

pub struct BashTool;

#[derive(Debug, Deserialize)]
struct BashParams {
    command: String,
    #[serde(default)]
    timeout: Option<u64>,
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Executes a bash command in the project directory and returns the output."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The bash command to execute"
                },
                "timeout": {
                    "type": "number",
                    "description": "Optional timeout in seconds (default: 120)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: serde_json::Value, working_dir: &Path) -> Result<String> {
        let params: BashParams = serde_json::from_value(params)
            .context("Invalid parameters for bash")?;

        let timeout = tokio::time::Duration::from_secs(params.timeout.unwrap_or(120));

        let output = tokio::time::timeout(
            timeout,
            Command::new("sh")
                .arg("-c")
                .arg(&params.command)
                .current_dir(working_dir)
                .output()
        )
        .await
        .context("Command timed out")?
        .context("Failed to execute command")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut result = String::new();
        if !stdout.is_empty() {
            result.push_str(&stdout);
        }
        if !stderr.is_empty() {
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&stderr);
        }

        if !output.status.success() {
            result.push_str(&format!("\nCommand exited with status: {}", output.status));
        }

        Ok(result)
    }
}
