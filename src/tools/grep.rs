use anyhow::Result;
use async_trait::async_trait;
use grep::regex::RegexMatcher;
use grep::searcher::sinks::UTF8;
use grep::searcher::Searcher;
use ignore::WalkBuilder;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::{Tool, ToolContext};

#[derive(Debug, Deserialize)]
struct GrepParams {
    /// The regex pattern to search for
    pattern: String,
    /// Optional path to search in (file or directory). Defaults to working directory.
    #[serde(default)]
    path: Option<String>,
    /// Optional glob pattern to filter files (e.g., "*.rs", "*.ts")
    #[serde(default)]
    include: Option<String>,
    /// Maximum number of results to return
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    50
}

pub struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Fast content search tool that works with any codebase size. \
         Supports full regex syntax (e.g., \"fn.*test\", \"class\\s+\\w+\"). \
         Use the include parameter to filter files by pattern. \
         Returns matching lines with file paths and line numbers."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The regex pattern to search for in file contents"
                },
                "path": {
                    "type": "string",
                    "description": "Optional path to search in (file or directory). Defaults to working directory."
                },
                "include": {
                    "type": "string",
                    "description": "Optional glob pattern to filter files (e.g., \"*.rs\", \"*.{ts,tsx}\")"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of results to return. Defaults to 50."
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(&self, params: serde_json::Value, ctx: &ToolContext<'_>) -> Result<String> {
        let params: GrepParams = serde_json::from_value(params)?;

        // Determine search path
        let search_path = if let Some(ref path) = params.path {
            if path.starts_with('/') {
                PathBuf::from(path)
            } else {
                ctx.working_dir.join(path)
            }
        } else {
            ctx.working_dir.to_path_buf()
        };

        // Build the regex matcher
        let matcher = match RegexMatcher::new(&params.pattern) {
            Ok(m) => m,
            Err(e) => return Ok(format!("Invalid regex pattern: {}", e)),
        };

        // Collect results
        let results: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let limit = params.limit;

        // Build file walker
        let mut walker_builder = WalkBuilder::new(&search_path);
        walker_builder
            .hidden(false)
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true);

        // Apply include filter if provided
        if let Some(ref include) = params.include {
            // Create a type matcher for the include pattern
            let mut types_builder = ignore::types::TypesBuilder::new();
            types_builder.add("custom", include)?;
            types_builder.select("custom");
            walker_builder.types(types_builder.build()?);
        }

        let walker = walker_builder.build();

        for entry in walker.filter_map(|e| e.ok()) {
            let path = entry.path();

            // Skip directories
            if !path.is_file() {
                continue;
            }

            // Check if we've hit the limit
            {
                let results_guard = results.lock().unwrap();
                if results_guard.len() >= limit {
                    break;
                }
            }

            let results_clone = Arc::clone(&results);
            let working_dir = ctx.working_dir.to_path_buf();
            let path_buf = path.to_path_buf();

            // Search this file
            let mut searcher = Searcher::new();
            let _ = searcher.search_path(
                &matcher,
                &path_buf,
                UTF8(|line_num, line| {
                    let mut results_guard = results_clone.lock().unwrap();
                    if results_guard.len() < limit {
                        let relative_path = path_buf
                            .strip_prefix(&working_dir)
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|_| path_buf.to_string_lossy().to_string());

                        results_guard.push(format!(
                            "{}:{}: {}",
                            relative_path,
                            line_num,
                            line.trim()
                        ));
                    }
                    Ok(results_guard.len() < limit)
                }),
            );
        }

        let results = results.lock().unwrap();

        if results.is_empty() {
            return Ok(format!("No matches found for pattern: {}", params.pattern));
        }

        let truncated = if results.len() >= limit {
            format!("\n\n(Results truncated at {} matches)", limit)
        } else {
            String::new()
        };

        Ok(format!(
            "Found {} matches for '{}':\n\n{}{}",
            results.len(),
            params.pattern,
            results.join("\n"),
            truncated
        ))
    }
}
