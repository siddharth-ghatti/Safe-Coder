use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use std::path::Path;

use super::{Tool, ToolContext};

pub struct EditTool;

#[derive(Debug, Deserialize)]
struct EditParams {
    file_path: String,
    old_string: String,
    new_string: String,
    #[serde(default)]
    replace_all: bool,
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit_file"
    }

    fn description(&self) -> &str {
        "Performs exact string replacements in files. The old_string must match exactly."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "The path to the file to edit (relative to project root)"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact text to replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The text to replace it with"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default: false)"
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext<'_>) -> Result<String> {
        let params: EditParams = serde_json::from_value(params)
            .context("Invalid parameters for edit_file")?;

        let file_path = ctx.working_dir.join(&params.file_path);

        if !file_path.exists() {
            anyhow::bail!("File not found: {}", params.file_path);
        }

        let content = std::fs::read_to_string(&file_path)
            .context("Failed to read file")?;

        let new_content = if params.replace_all {
            content.replace(&params.old_string, &params.new_string)
        } else {
            // Replace only the first occurrence
            if let Some(pos) = content.find(&params.old_string) {
                let mut result = String::new();
                result.push_str(&content[..pos]);
                result.push_str(&params.new_string);
                result.push_str(&content[pos + params.old_string.len()..]);
                result
            } else {
                anyhow::bail!("String not found in file: {}", params.old_string);
            }
        };

        std::fs::write(&file_path, &new_content)
            .context("Failed to write file")?;

        Ok(format!("Successfully edited {}", params.file_path))
    }
}
