//! File picker dropup component for @file mentions
//!
//! Provides a visual file browser that appears when typing @ in the input.

use std::fs;
use std::path::Path;

/// A file entry in the picker
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Display name
    pub name: String,
    /// Full relative path
    pub path: String,
    /// Whether this is a directory
    pub is_dir: bool,
    /// File size (for files only)
    pub size: Option<u64>,
}

/// File picker state
#[derive(Debug, Clone)]
pub struct FilePicker {
    /// Whether the picker is visible
    pub visible: bool,
    /// Current directory being browsed (relative to cwd)
    pub current_dir: String,
    /// List of entries in current directory
    pub entries: Vec<FileEntry>,
    /// Currently selected index
    pub selected: usize,
    /// Filter/search text
    pub filter: String,
    /// Files that have been selected/attached
    pub attached_files: Vec<String>,
}

impl Default for FilePicker {
    fn default() -> Self {
        Self::new()
    }
}

impl FilePicker {
    pub fn new() -> Self {
        Self {
            visible: false,
            current_dir: String::new(),
            entries: Vec::new(),
            selected: 0,
            filter: String::new(),
            attached_files: Vec::new(),
        }
    }

    /// Open the file picker and load the current directory
    pub fn open(&mut self, cwd: &Path) {
        self.visible = true;
        self.current_dir = String::new();
        self.filter = String::new();
        self.selected = 0;
        self.load_directory(cwd);
    }

    /// Close the file picker
    pub fn close(&mut self) {
        self.visible = false;
        self.entries.clear();
        self.filter.clear();
    }

    /// Load entries from a directory
    pub fn load_directory(&mut self, cwd: &Path) {
        self.entries.clear();
        self.selected = 0;

        let search_path = if self.current_dir.is_empty() {
            cwd.to_path_buf()
        } else {
            cwd.join(&self.current_dir)
        };

        // Add parent directory option if not at root
        if !self.current_dir.is_empty() {
            self.entries.push(FileEntry {
                name: "..".to_string(),
                path: "..".to_string(),
                is_dir: true,
                size: None,
            });
        }

        if let Ok(read_dir) = fs::read_dir(&search_path) {
            let mut entries: Vec<FileEntry> = read_dir
                .filter_map(|e| e.ok())
                .filter_map(|entry| {
                    let name = entry.file_name().to_string_lossy().to_string();

                    // Skip hidden files unless filter starts with .
                    if name.starts_with('.') && !self.filter.starts_with('.') {
                        return None;
                    }

                    let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    let size = if is_dir {
                        None
                    } else {
                        entry.metadata().ok().map(|m| m.len())
                    };

                    let path = if self.current_dir.is_empty() {
                        name.clone()
                    } else {
                        format!("{}/{}", self.current_dir, name)
                    };

                    Some(FileEntry {
                        name,
                        path,
                        is_dir,
                        size,
                    })
                })
                .collect();

            // Sort: directories first, then alphabetically
            entries.sort_by(|a, b| match (a.is_dir, b.is_dir) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
            });

            self.entries.extend(entries);
        }
    }

    /// Get filtered entries based on current filter
    pub fn filtered_entries(&self) -> Vec<&FileEntry> {
        if self.filter.is_empty() {
            self.entries.iter().collect()
        } else {
            let filter_lower = self.filter.to_lowercase();
            self.entries
                .iter()
                .filter(|e| e.name.to_lowercase().contains(&filter_lower))
                .collect()
        }
    }

    /// Update filter text
    pub fn set_filter(&mut self, filter: String) {
        self.filter = filter;
        self.selected = 0;
    }

    /// Add character to filter
    pub fn filter_push(&mut self, c: char) {
        self.filter.push(c);
        self.selected = 0;
    }

    /// Remove last character from filter
    pub fn filter_pop(&mut self) {
        self.filter.pop();
        self.selected = 0;
    }

    /// Move selection up
    pub fn select_up(&mut self) {
        let count = self.filtered_entries().len();
        if count > 0 && self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Move selection down
    pub fn select_down(&mut self) {
        let count = self.filtered_entries().len();
        if count > 0 && self.selected < count - 1 {
            self.selected += 1;
        }
    }

    /// Get currently selected entry
    pub fn selected_entry(&self) -> Option<&FileEntry> {
        self.filtered_entries().get(self.selected).copied()
    }

    /// Select current entry (enter directory or attach file)
    /// Returns Some(path) if a file was selected, None if navigating
    pub fn select_current(&mut self, cwd: &Path) -> Option<String> {
        let entry = self.selected_entry()?.clone();

        if entry.is_dir {
            if entry.name == ".." {
                // Go up one directory
                if let Some(pos) = self.current_dir.rfind('/') {
                    self.current_dir = self.current_dir[..pos].to_string();
                } else {
                    self.current_dir = String::new();
                }
            } else {
                // Enter directory
                self.current_dir = entry.path.clone();
            }
            self.filter.clear();
            self.load_directory(cwd);
            None
        } else {
            // File selected - attach it
            let path = entry.path.clone();
            if !self.attached_files.contains(&path) {
                self.attached_files.push(path.clone());
            }
            self.close();
            Some(path)
        }
    }

    /// Remove an attached file
    pub fn remove_attached(&mut self, path: &str) {
        self.attached_files.retain(|p| p != path);
    }

    /// Clear all attached files
    pub fn clear_attached(&mut self) {
        self.attached_files.clear();
    }

    /// Format file size for display
    pub fn format_size(size: u64) -> String {
        if size < 1024 {
            format!("{}B", size)
        } else if size < 1024 * 1024 {
            format!("{:.1}K", size as f64 / 1024.0)
        } else if size < 1024 * 1024 * 1024 {
            format!("{:.1}M", size as f64 / (1024.0 * 1024.0))
        } else {
            format!("{:.1}G", size as f64 / (1024.0 * 1024.0 * 1024.0))
        }
    }
}
