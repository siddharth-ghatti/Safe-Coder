use anyhow::Result;
use async_trait::async_trait;
use glob::glob;
use serde::Deserialize;
use std::path::PathBuf;

use super::{Tool, ToolContext};

#[derive(Debug, Deserialize)]
struct GlobParams {
    /// The glob pattern to match files against (e.g., "**/*.rs", "src/**/*.ts")
    pattern: String,
    /// Optional directory path to search in. Defaults to working directory.
    #[serde(default)]
    path: Option<String>,
}

pub struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Fast file pattern matching tool that works with any codebase size. \
         Supports glob patterns like \"**/*.rs\" or \"src/**/*.ts\". \
         Returns matching file paths sorted by modification time."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The glob pattern to match files against (e.g., \"**/*.rs\", \"src/**/*.ts\")"
                },
                "path": {
                    "type": "string",
                    "description": "Optional directory path to search in. Defaults to current working directory."
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext<'_>) -> Result<String> {
        let params: GlobParams = serde_json::from_value(params)?;

        // Determine the base path
        let base_path = if let Some(ref path) = params.path {
            if path.starts_with('/') {
                PathBuf::from(path)
            } else {
                ctx.working_dir.join(path)
            }
        } else {
            ctx.working_dir.to_path_buf()
        };

        // Construct the full glob pattern
        let full_pattern = base_path.join(&params.pattern);
        let pattern_str = full_pattern.to_string_lossy();

        // Collect matching files with metadata for sorting
        let mut matches: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

        for entry in glob(&pattern_str)? {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        let mtime = path
                            .metadata()
                            .and_then(|m| m.modified())
                            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                        matches.push((path, mtime));
                    }
                }
                Err(e) => {
                    tracing::warn!("Glob error for entry: {}", e);
                }
            }
        }

        // Sort by modification time (most recent first)
        matches.sort_by(|a, b| b.1.cmp(&a.1));

        if matches.is_empty() {
            return Ok(format!("No files matched pattern: {}", params.pattern));
        }

        // Format output - show relative paths when possible
        let output: Vec<String> = matches
            .iter()
            .map(|(path, _)| {
                path.strip_prefix(ctx.working_dir)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| path.to_string_lossy().to_string())
            })
            .collect();

        Ok(format!(
            "Found {} files matching '{}':\n{}",
            output.len(),
            params.pattern,
            output.join("\n")
        ))
    }
}
