use anyhow::Result;
use async_trait::async_trait;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

use super::{Tool, ToolContext};

#[derive(Debug, Deserialize)]
struct ListParams {
    /// The directory path to list (must be absolute path)
    path: String,
    /// Optional array of glob patterns to ignore
    #[serde(default)]
    ignore: Vec<String>,
    /// Whether to show hidden files (default: false)
    #[serde(default)]
    show_hidden: bool,
    /// Maximum depth to recurse (default: 1, meaning just the directory contents)
    #[serde(default = "default_depth")]
    depth: usize,
}

fn default_depth() -> usize {
    1
}

pub struct ListTool;

#[async_trait]
impl Tool for ListTool {
    fn name(&self) -> &str {
        "list"
    }

    fn description(&self) -> &str {
        "Lists files and directories in a given path. \
         Returns a tree-like structure showing the contents. \
         Use this to explore directory structure."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The directory path to list (absolute or relative to working directory)"
                },
                "ignore": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Optional array of glob patterns to ignore (e.g., [\"node_modules\", \"*.log\"])"
                },
                "show_hidden": {
                    "type": "boolean",
                    "description": "Whether to show hidden files (starting with dot). Defaults to false."
                },
                "depth": {
                    "type": "integer",
                    "description": "Maximum depth to recurse. 1 = just directory contents, 2+ = include subdirectories. Defaults to 1."
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext<'_>) -> Result<String> {
        let params: ListParams = serde_json::from_value(params)?;

        // Resolve path
        let target_path = if params.path.starts_with('/') {
            PathBuf::from(&params.path)
        } else {
            ctx.working_dir.join(&params.path)
        };

        if !target_path.exists() {
            return Ok(format!("Path does not exist: {}", params.path));
        }

        if !target_path.is_dir() {
            return Ok(format!("Path is not a directory: {}", params.path));
        }

        // Build the listing
        let mut output = Vec::new();
        output.push(format!("{}:", target_path.display()));

        list_directory(
            &target_path,
            &params.ignore,
            params.show_hidden,
            params.depth,
            0,
            "",
            &mut output,
        )?;

        Ok(output.join("\n"))
    }
}

fn list_directory(
    path: &PathBuf,
    ignore_patterns: &[String],
    show_hidden: bool,
    max_depth: usize,
    current_depth: usize,
    prefix: &str,
    output: &mut Vec<String>,
) -> Result<()> {
    if current_depth >= max_depth {
        return Ok(());
    }

    let mut entries: Vec<_> = fs::read_dir(path)?.filter_map(|e| e.ok()).collect();

    // Sort entries: directories first, then files, both alphabetically
    entries.sort_by(|a, b| {
        let a_is_dir = a.path().is_dir();
        let b_is_dir = b.path().is_dir();
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });

    let total = entries.len();

    for (idx, entry) in entries.iter().enumerate() {
        let file_name = entry.file_name().to_string_lossy().to_string();
        let is_last = idx == total - 1;

        // Skip hidden files unless show_hidden is true
        if !show_hidden && file_name.starts_with('.') {
            continue;
        }

        // Check ignore patterns
        let should_ignore = ignore_patterns.iter().any(|pattern| {
            glob::Pattern::new(pattern)
                .map(|p| p.matches(&file_name))
                .unwrap_or(false)
        });

        if should_ignore {
            continue;
        }

        let is_dir = entry.path().is_dir();
        let connector = if is_last { "└── " } else { "├── " };
        let icon = if is_dir { "📁" } else { "📄" };

        output.push(format!("{}{}{} {}", prefix, connector, icon, file_name));

        // Recurse into directories
        if is_dir && current_depth + 1 < max_depth {
            let new_prefix = if is_last {
                format!("{}    ", prefix)
            } else {
                format!("{}│   ", prefix)
            };

            list_directory(
                &entry.path(),
                ignore_patterns,
                show_hidden,
                max_depth,
                current_depth + 1,
                &new_prefix,
                output,
            )?;
        }
    }

    Ok(())
}
