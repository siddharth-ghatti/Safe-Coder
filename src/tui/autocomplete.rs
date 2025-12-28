//! Shell command autocomplete functionality
//!
//! Provides Tab completion for:
//! - Commands from PATH
//! - File and directory paths
//! - Built-in shell commands
//! - @file mentions for AI context

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// Autocomplete state and suggestions
#[derive(Debug, Clone)]
pub struct Autocomplete {
    /// Current suggestions
    pub suggestions: Vec<String>,
    /// Currently selected suggestion index
    pub selected: usize,
    /// Whether autocomplete popup is visible
    pub visible: bool,
    /// The prefix being completed
    pub prefix: String,
    /// Cached commands from PATH
    path_commands: HashSet<String>,
    /// Whether PATH commands have been loaded
    path_loaded: bool,
    /// Whether we're completing a @file mention
    pub completing_at_mention: bool,
}

impl Default for Autocomplete {
    fn default() -> Self {
        Self::new()
    }
}

impl Autocomplete {
    pub fn new() -> Self {
        Self {
            suggestions: Vec::new(),
            selected: 0,
            visible: false,
            prefix: String::new(),
            path_commands: HashSet::new(),
            path_loaded: false,
            completing_at_mention: false,
        }
    }

    /// Load commands from PATH (lazy loading)
    fn ensure_path_loaded(&mut self) {
        if self.path_loaded {
            return;
        }

        if let Ok(path_var) = env::var("PATH") {
            for path_dir in env::split_paths(&path_var) {
                if let Ok(entries) = fs::read_dir(&path_dir) {
                    for entry in entries.flatten() {
                        if let Ok(file_type) = entry.file_type() {
                            if file_type.is_file() || file_type.is_symlink() {
                                if let Some(name) = entry.file_name().to_str() {
                                    // Check if executable (on Unix)
                                    #[cfg(unix)]
                                    {
                                        use std::os::unix::fs::PermissionsExt;
                                        if let Ok(metadata) = entry.metadata() {
                                            if metadata.permissions().mode() & 0o111 != 0 {
                                                self.path_commands.insert(name.to_string());
                                            }
                                        }
                                    }
                                    #[cfg(not(unix))]
                                    {
                                        self.path_commands.insert(name.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        self.path_loaded = true;
    }

    /// Built-in shell commands
    fn builtin_commands() -> Vec<&'static str> {
        vec![
            "cd", "pwd", "exit", "quit", "clear", "history", "export", "env", "help",
        ]
    }

    /// Generate completions for the given input
    pub fn complete(&mut self, input: &str, cwd: &Path) {
        self.ensure_path_loaded();
        self.suggestions.clear();
        self.selected = 0;
        self.completing_at_mention = false;

        if input.is_empty() {
            self.visible = false;
            return;
        }

        // Check if we're completing an @file mention
        // Find the last @ that might be a file mention
        if let Some(at_pos) = input.rfind('@') {
            let after_at = &input[at_pos + 1..];
            // If there's no space after @, we're completing a file mention
            if !after_at.contains(' ') {
                self.completing_at_mention = true;
                self.prefix = format!("@{}", after_at);
                self.complete_at_mention(after_at, cwd);
                self.visible = !self.suggestions.is_empty();
                return;
            }
        }

        // Parse the input to find what we're completing
        let parts: Vec<&str> = input.split_whitespace().collect();
        let completing_command = parts.len() <= 1 && !input.ends_with(' ');

        if completing_command {
            // Complete command name
            let prefix = parts.first().copied().unwrap_or("");
            self.prefix = prefix.to_string();
            self.complete_command(prefix);
        } else {
            // Complete file/directory path
            let last_part = if input.ends_with(' ') {
                ""
            } else {
                parts.last().copied().unwrap_or("")
            };
            self.prefix = last_part.to_string();
            self.complete_path(last_part, cwd);
        }

        self.visible = !self.suggestions.is_empty();
    }

    /// Complete @file mentions for AI context
    fn complete_at_mention(&mut self, partial: &str, cwd: &Path) {
        let (dir_path, file_prefix) = if partial.contains('/') {
            let path = Path::new(partial);
            if partial.ends_with('/') {
                (partial.to_string(), String::new())
            } else {
                let parent = path
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let file = path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();
                (parent, file)
            }
        } else {
            (String::new(), partial.to_string())
        };

        // Resolve the directory to search
        let search_dir = if dir_path.is_empty() {
            cwd.to_path_buf()
        } else {
            cwd.join(&dir_path)
        };

        // Read directory entries
        if let Ok(entries) = fs::read_dir(&search_dir) {
            let prefix_lower = file_prefix.to_lowercase();

            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    // Skip hidden files unless explicitly typing them
                    if name.starts_with('.') && !file_prefix.starts_with('.') {
                        continue;
                    }

                    if name.to_lowercase().starts_with(&prefix_lower) {
                        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

                        // Build the completion string with @ prefix
                        let path_part = if dir_path.is_empty() {
                            if is_dir {
                                format!("{}/", name)
                            } else {
                                name.to_string()
                            }
                        } else {
                            let sep = if dir_path.ends_with('/') { "" } else { "/" };
                            if is_dir {
                                format!("{}{}{}/", dir_path, sep, name)
                            } else {
                                format!("{}{}{}", dir_path, sep, name)
                            }
                        };

                        self.suggestions.push(format!("@{}", path_part));
                    }
                }
            }
        }

        // Sort: directories first, then alphabetically
        self.suggestions.sort_by(|a, b| {
            let a_is_dir = a.ends_with('/');
            let b_is_dir = b.ends_with('/');
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.to_lowercase().cmp(&b.to_lowercase()),
            }
        });

        // Limit suggestions
        self.suggestions.truncate(15);
    }

    /// Complete command names
    fn complete_command(&mut self, prefix: &str) {
        let prefix_lower = prefix.to_lowercase();

        // Add matching built-in commands
        for cmd in Self::builtin_commands() {
            if cmd.starts_with(&prefix_lower) {
                self.suggestions.push(cmd.to_string());
            }
        }

        // Add matching PATH commands
        let mut path_matches: Vec<_> = self
            .path_commands
            .iter()
            .filter(|cmd| cmd.to_lowercase().starts_with(&prefix_lower))
            .cloned()
            .collect();
        path_matches.sort();
        self.suggestions.extend(path_matches);

        // Remove duplicates while preserving order
        let mut seen = HashSet::new();
        self.suggestions.retain(|s| seen.insert(s.clone()));

        // Limit suggestions
        self.suggestions.truncate(10);
    }

    /// Complete file/directory paths
    fn complete_path(&mut self, partial: &str, cwd: &Path) {
        let (dir_path, file_prefix) = if partial.contains('/') || partial.contains('\\') {
            let path = Path::new(partial);
            if partial.ends_with('/') || partial.ends_with('\\') {
                (partial.to_string(), String::new())
            } else {
                let parent = path
                    .parent()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_default();
                let file = path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_default();
                (parent, file)
            }
        } else {
            (String::new(), partial.to_string())
        };

        // Resolve the directory to search
        let search_dir = if dir_path.is_empty() {
            cwd.to_path_buf()
        } else if dir_path.starts_with('~') {
            if let Some(home) = dirs::home_dir() {
                if dir_path == "~" {
                    home
                } else {
                    home.join(&dir_path[2..])
                }
            } else {
                return;
            }
        } else if Path::new(&dir_path).is_absolute() {
            PathBuf::from(&dir_path)
        } else {
            cwd.join(&dir_path)
        };

        // Read directory entries
        if let Ok(entries) = fs::read_dir(&search_dir) {
            let prefix_lower = file_prefix.to_lowercase();

            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.to_lowercase().starts_with(&prefix_lower) {
                        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);

                        // Build the completion string
                        let completion = if dir_path.is_empty() {
                            if is_dir {
                                format!("{}/", name)
                            } else {
                                name.to_string()
                            }
                        } else {
                            let sep = if dir_path.ends_with('/') { "" } else { "/" };
                            if is_dir {
                                format!("{}{}{}/", dir_path, sep, name)
                            } else {
                                format!("{}{}{}", dir_path, sep, name)
                            }
                        };

                        self.suggestions.push(completion);
                    }
                }
            }
        }

        // Sort: directories first, then alphabetically
        self.suggestions.sort_by(|a, b| {
            let a_is_dir = a.ends_with('/');
            let b_is_dir = b.ends_with('/');
            match (a_is_dir, b_is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.to_lowercase().cmp(&b.to_lowercase()),
            }
        });

        // Limit suggestions
        self.suggestions.truncate(10);
    }

    /// Select next suggestion
    pub fn next(&mut self) {
        if !self.suggestions.is_empty() {
            self.selected = (self.selected + 1) % self.suggestions.len();
        }
    }

    /// Select previous suggestion
    pub fn prev(&mut self) {
        if !self.suggestions.is_empty() {
            if self.selected == 0 {
                self.selected = self.suggestions.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
    }

    /// Get the currently selected suggestion
    pub fn current(&self) -> Option<&str> {
        self.suggestions.get(self.selected).map(|s| s.as_str())
    }

    /// Apply the current suggestion to the input
    pub fn apply(&self, input: &str) -> Option<String> {
        let suggestion = self.current()?;

        // Find where to insert the completion
        let parts: Vec<&str> = input.split_whitespace().collect();

        if parts.len() <= 1 && !input.ends_with(' ') {
            // Replacing the command
            Some(suggestion.to_string())
        } else {
            // Replacing the last argument
            let last_start = input
                .rfind(|c: char| c.is_whitespace())
                .map(|i| i + 1)
                .unwrap_or(0);
            let mut result = input[..last_start].to_string();
            result.push_str(suggestion);
            Some(result)
        }
    }

    /// Hide the autocomplete popup
    pub fn hide(&mut self) {
        self.visible = false;
        self.suggestions.clear();
        self.selected = 0;
    }

    /// Check if there's only one suggestion (for immediate completion)
    pub fn single_match(&self) -> bool {
        self.suggestions.len() == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env::temp_dir;

    #[test]
    fn test_command_completion() {
        let mut ac = Autocomplete::new();
        ac.complete("l", &temp_dir());
        // Should have suggestions starting with 'l'
        assert!(ac.suggestions.iter().any(|s| s.starts_with("l")));
    }

    #[test]
    fn test_builtin_completion() {
        let mut ac = Autocomplete::new();
        ac.complete("cd", &temp_dir());
        assert!(ac.suggestions.contains(&"cd".to_string()));
    }
}
