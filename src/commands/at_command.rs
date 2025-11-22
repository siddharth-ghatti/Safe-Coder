use anyhow::{Context, Result};
use glob::glob;
use std::path::PathBuf;
use crate::commands::CommandResult;
use crate::session::Session;

/// At-command for attaching file context
#[derive(Debug, Clone)]
pub struct AtCommand {
    pub original_text: String,
    pub files: Vec<String>,
}

impl AtCommand {
    /// Parse at-commands from input
    /// Examples: "@file.rs", "@src/**/*.rs", "Check @main.rs for errors"
    pub fn parse(input: &str) -> Option<Self> {
        let mut files = Vec::new();
        let mut in_at_command = false;
        let mut current_pattern = String::new();

        for (i, ch) in input.chars().enumerate() {
            if ch == '@' {
                // Check if this is the start of an at-command
                // Not if it's part of an email or preceded by alphanumeric
                let prev_char = if i > 0 { input.chars().nth(i - 1) } else { None };
                let should_start = match prev_char {
                    Some(c) if c.is_alphanumeric() => false,
                    _ => true,
                };

                if should_start {
                    if !current_pattern.is_empty() {
                        files.push(current_pattern.clone());
                        current_pattern.clear();
                    }
                    in_at_command = true;
                }
            } else if in_at_command {
                if ch.is_whitespace() || ch == ',' {
                    if !current_pattern.is_empty() {
                        files.push(current_pattern.clone());
                        current_pattern.clear();
                    }
                    in_at_command = false;
                } else {
                    current_pattern.push(ch);
                }
            }
        }

        // Don't forget the last pattern
        if !current_pattern.is_empty() {
            files.push(current_pattern);
        }

        if files.is_empty() {
            None
        } else {
            Some(AtCommand {
                original_text: input.to_string(),
                files,
            })
        }
    }

    /// Expand file patterns to actual file paths
    pub fn expand_files(&self, base_path: &PathBuf) -> Result<Vec<PathBuf>> {
        let mut expanded = Vec::new();

        for pattern in &self.files {
            // If it's a glob pattern
            if pattern.contains('*') || pattern.contains('?') {
                let full_pattern = base_path.join(pattern);
                let pattern_str = full_pattern.to_str()
                    .context("Invalid path pattern")?;

                for entry in glob(pattern_str)? {
                    match entry {
                        Ok(path) => expanded.push(path),
                        Err(e) => tracing::warn!("Failed to read glob entry: {}", e),
                    }
                }
            } else {
                // Direct file reference
                let path = base_path.join(pattern);
                if path.exists() {
                    expanded.push(path);
                } else {
                    tracing::warn!("File not found: {}", pattern);
                }
            }
        }

        Ok(expanded)
    }
}

/// Execute at-command by reading files and appending to message
pub async fn execute_at_command(cmd: AtCommand, session: &mut Session) -> Result<CommandResult> {
    let sandbox_dir = session.get_sandbox_dir()?;
    let files = cmd.expand_files(&sandbox_dir)?;

    if files.is_empty() {
        return Ok(CommandResult::Message(
            format!("⚠ No files found matching: {}", cmd.files.join(", "))
        ));
    }

    let mut context = String::new();
    context.push_str(&format!("\n\n--- Attached Files ({}) ---\n", files.len()));

    for file_path in &files {
        let relative_path = file_path.strip_prefix(&sandbox_dir)
            .unwrap_or(file_path);

        match tokio::fs::read_to_string(file_path).await {
            Ok(content) => {
                context.push_str(&format!("\n## File: {}\n", relative_path.display()));
                context.push_str("```\n");
                context.push_str(&content);
                context.push_str("\n```\n");
            },
            Err(e) => {
                tracing::warn!("Failed to read {}: {}", relative_path.display(), e);
                context.push_str(&format!("\n## File: {} (Error: {})\n", relative_path.display(), e));
            }
        }
    }

    // Replace @commands with empty string and append file contents
    let mut modified_text = cmd.original_text.clone();
    for pattern in &cmd.files {
        modified_text = modified_text.replace(&format!("@{}", pattern), "");
    }

    // Clean up extra whitespace
    modified_text = modified_text.split_whitespace().collect::<Vec<_>>().join(" ");

    // Append context
    modified_text.push_str(&context);

    Ok(CommandResult::ModifiedInput(modified_text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_file() {
        let cmd = AtCommand::parse("Check @main.rs for errors");
        assert!(cmd.is_some());
        let cmd = cmd.unwrap();
        assert_eq!(cmd.files.len(), 1);
        assert_eq!(cmd.files[0], "main.rs");
    }

    #[test]
    fn test_parse_multiple_files() {
        let cmd = AtCommand::parse("Review @file1.rs and @file2.rs");
        assert!(cmd.is_some());
        let cmd = cmd.unwrap();
        assert_eq!(cmd.files.len(), 2);
    }

    #[test]
    fn test_parse_glob_pattern() {
        let cmd = AtCommand::parse("@src/**/*.rs");
        assert!(cmd.is_some());
        let cmd = cmd.unwrap();
        assert_eq!(cmd.files[0], "src/**/*.rs");
    }

    #[test]
    fn test_ignore_email() {
        let cmd = AtCommand::parse("Email me at user@example.com");
        // Should not parse email addresses as at-commands
        assert!(cmd.is_none());
    }
}
